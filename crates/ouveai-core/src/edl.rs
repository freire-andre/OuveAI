//! Parte 5: legenda de vídeo editado a partir dos brutos já transcritos e de um `edl.json`.
//!
//! - `edl.json`: `[{"file": "bruto1.mp4", "in": 12.40, "out": 25.10, "position": 0.00}, ...]`,
//!   em segundos e sem mudança de velocidade.
//! - Cada bruto precisa do `.json` de mesmo nome-base (gerado pela transcrição).
//! - Entra a palavra cujo início está em `[in, out)`; tempo final = tempo no bruto − in +
//!   position; fim que passa de `out` é cortado em `out`. Nenhum bloco atravessa um corte.
//! - Verificação: transcrição `--check` do vídeo final, alinhada por sequência com a legenda.

use crate::subs::{self, Cue};
use crate::transcribe::{self as tr, Transcript};
use crate::words::{Pause, Word, ms};
use similar::{Algorithm, DiffTag, capture_diff_slices};
use std::path::{Path, PathBuf};

/// Desvio acima do qual um trecho é listado (prompt.txt, Parte 5).
pub const DESVIO_LISTAR: f64 = 0.3;
/// Referência do prompt para o desvio mediano.
pub const DESVIO_REFERENCIA: f64 = 0.12;
/// Desvios de um corte mais espalhados que isto (desvio absoluto mediano) não são um
/// deslocamento simples do corte.
const ESPALHAMENTO_MAX: f64 = 0.15;
/// Mínimo de pares casados num corte para propor mudar o corte inteiro.
const MIN_PARES_CORTE: usize = 3;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Cut {
    pub file: String,
    #[serde(rename = "in")]
    pub in_: f64,
    pub out: f64,
    pub position: f64,
}

impl Cut {
    pub fn span(&self) -> f64 {
        self.out - self.in_
    }
    /// Fim do corte no vídeo final.
    pub fn final_end(&self) -> f64 {
        self.position + self.span()
    }
}

/// Intervalo do vídeo final sem nenhum corte (título, transição).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct Gap {
    pub start: f64,
    pub end: f64,
}

/// Lê o `edl.json`. Erro de formato vira mensagem clara.
pub fn parse(text: &str) -> Result<Vec<Cut>, String> {
    let cuts: Vec<Cut> = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("edl.json inválido: {e}. O formato é [{{\"file\": \"bruto1.mp4\", \"in\": 12.4, \"out\": 25.1, \"position\": 0}}, ...]"))?;
    if cuts.is_empty() {
        return Err("o edl.json não tem nenhum corte".into());
    }
    Ok(cuts)
}

pub fn read(path: &Path) -> Result<Vec<Cut>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("não foi possível ler {}: {e}", path.display()))?;
    parse(&text)
}

/// Valida antes de tudo: `out > in`, valores não negativos e nenhuma sobreposição no
/// vídeo final. Devolve as lacunas (permitidas) ou a lista de erros. Os cortes são
/// numerados a partir de 1, na ordem do arquivo.
pub fn validate(cuts: &[Cut]) -> Result<Vec<Gap>, Vec<String>> {
    let mut errors = Vec::new();
    for (i, c) in cuts.iter().enumerate() {
        let n = i + 1;
        if ![c.in_, c.out, c.position].iter().all(|v| v.is_finite()) {
            errors.push(format!("corte {n} ({}): in, out e position precisam ser números", c.file));
            continue;
        }
        if c.file.trim().is_empty() {
            errors.push(format!("corte {n}: \"file\" está vazio"));
        }
        if c.out <= c.in_ {
            errors.push(format!("corte {n} ({}): out ({:.3}) precisa ser maior que in ({:.3})", c.file, c.out, c.in_));
        }
        if c.in_ < 0.0 || c.position < 0.0 {
            errors.push(format!("corte {n} ({}): in e position não podem ser negativos", c.file));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let order = by_position(cuts);
    let mut gaps = Vec::new();
    let mut prev_end = 0.0;
    let mut prev: Option<usize> = None;
    for &i in &order {
        let c = &cuts[i];
        if let Some(p) = prev
            && c.position < prev_end - 0.0005
        {
            errors.push(format!(
                "corte {} ({}) sobrepõe o corte {} ({}) no vídeo final, entre {} e {}",
                i + 1,
                c.file,
                p + 1,
                cuts[p].file,
                clock(c.position),
                clock(prev_end.min(c.final_end()))
            ));
        }
        if c.position > prev_end + 0.0005 {
            gaps.push(Gap { start: ms(prev_end), end: ms(c.position) });
        }
        if c.final_end() > prev_end {
            prev_end = c.final_end();
            prev = Some(i);
        }
    }
    if errors.is_empty() { Ok(gaps) } else { Err(errors) }
}

fn by_position(cuts: &[Cut]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..cuts.len()).collect();
    order.sort_by(|&a, &b| cuts[a].position.total_cmp(&cuts[b].position));
    order
}

/// `.json` de mesmo nome-base do bruto; caminho relativo é relativo à pasta do `edl.json`.
pub fn source_json(edl: &Path, file: &str) -> PathBuf {
    let f = Path::new(file);
    let f = if f.is_absolute() { f.to_path_buf() } else { edl.parent().unwrap_or(Path::new(".")).join(f) };
    tr::outputs_for(&f).json
}

/// Lê o `.json` de cada bruto (uma vez por arquivo). Falta de `.json` é erro claro.
pub fn load_sources(edl: &Path, cuts: &[Cut]) -> Result<Vec<Transcript>, Vec<String>> {
    let mut cache: Vec<(PathBuf, Transcript)> = Vec::new();
    let mut errors = Vec::new();
    let mut out = Vec::new();
    for c in cuts {
        let p = source_json(edl, &c.file);
        if let Some((_, t)) = cache.iter().find(|(q, _)| *q == p) {
            out.push(t.clone());
            continue;
        }
        if !p.is_file() {
            let msg = format!("falta {}: transcreva {} antes (o .json fica ao lado dele)", file_name(&p), c.file);
            if !errors.contains(&msg) {
                errors.push(msg);
            }
            continue;
        }
        match tr::read_json(&p) {
            Ok(t) => {
                cache.push((p, t.clone()));
                out.push(t);
            }
            Err(e) => errors.push(format!("{}: {e}", file_name(&p))),
        }
    }
    if errors.is_empty() { Ok(out) } else { Err(errors) }
}

/// Avisos que não impedem gerar: corte que passa do fim do bruto ou sem nenhuma palavra.
pub fn warnings(cuts: &[Cut], sources: &[Transcript]) -> Vec<String> {
    let mut w = Vec::new();
    for (i, (c, t)) in cuts.iter().zip(sources).enumerate() {
        if c.out > t.duration + 0.5 {
            w.push(format!("corte {} ({}): out ({:.3}) passa do fim do bruto ({:.3})", i + 1, c.file, c.out, t.duration));
        }
        if map_cut(c, t).0.is_empty() {
            w.push(format!("corte {} ({}): nenhuma palavra entre {:.3} e {:.3}", i + 1, c.file, c.in_, c.out));
        }
    }
    w
}

/// Palavras e pausas de um corte, já no tempo do vídeo final.
pub fn map_cut(c: &Cut, t: &Transcript) -> (Vec<Word>, Vec<Pause>) {
    let shift = c.position - c.in_;
    let words = t
        .words
        .iter()
        .filter(|w| w.start >= c.in_ && w.start < c.out)
        .map(|w| Word { word: w.word.clone(), start: ms(w.start + shift), end: ms(w.end.min(c.out) + shift) })
        .collect();
    let pauses = t
        .pauses
        .iter()
        .filter(|p| p.end > c.in_ && p.start < c.out)
        .map(|p| Pause { start: ms(p.start.max(c.in_) + shift), end: ms(p.end.min(c.out) + shift) })
        .collect();
    (words, pauses)
}

/// Uma palavra da legenda final e o corte de onde veio (índice no arquivo).
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct FinalWord {
    pub word: String,
    pub start: f64,
    pub end: f64,
    pub cut: usize,
}

#[derive(Clone, Debug)]
pub struct Built {
    pub cues: Vec<Cue>,
    pub words: Vec<FinalWord>,
}

/// Monta a legenda: a mesma `subs::segment` em cada corte, então nenhum bloco atravessa
/// um corte. O fim do último bloco de um corte não passa do fim do corte nem invade o
/// intervalo mínimo antes do primeiro bloco do corte seguinte.
pub fn build(cuts: &[Cut], sources: &[Transcript]) -> Built {
    let mut cues: Vec<Cue> = Vec::new();
    let mut words = Vec::new();
    for i in by_position(cuts) {
        let c = &cuts[i];
        let (ws, ps) = map_cut(c, &sources[i]);
        let mut cs = subs::segment(&ws, &ps);
        for cue in &mut cs {
            cue.end = cue.end.min(c.final_end()).max(cue.start + 0.001);
        }
        if let (Some(last), Some(first)) = (cues.last_mut(), cs.first()) {
            last.end = last.end.min(first.start - subs::INTERVALO_MIN).max(last.start + 0.001);
        }
        cues.extend(cs);
        words.extend(ws.into_iter().map(|w| FinalWord { word: w.word, start: w.start, end: w.end, cut: i }));
    }
    Built { cues, words }
}

/// `.srt` do vídeo final: `--out`, ou o nome do vídeo final quando ele é conhecido
/// (o player carrega sozinho), ou o nome-base do `edl.json`.
pub fn srt_path(edl: &Path, video: Option<&Path>) -> PathBuf {
    match video {
        Some(v) => v.with_extension("srt"),
        None => edl.with_extension("srt"),
    }
}

// ---------- verificação ----------

/// Por que um trecho desviou, e o que corrigir na origem.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Cause {
    /// O corte inteiro está deslocado: `by` > 0 quer dizer que a fala aparece mais tarde no
    /// vídeo final do que a legenda diz.
    Shift { by: f64 },
    /// O desvio muda ao longo do corte: mudança de velocidade ou bruto diferente.
    Drift { from: f64, to: f64 },
    /// Poucas palavras soltas: reconhecimento diferente ou pareamento errado.
    Isolated,
}

/// Trecho com desvio > `DESVIO_LISTAR`, no tempo do vídeo final.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Issue {
    pub cut: usize,
    pub start: f64,
    pub end: f64,
    pub words: usize,
    /// Maior desvio do trecho (com sinal).
    pub worst: f64,
    pub cause: Cause,
    /// A causa provável, em texto.
    pub why: String,
}

/// A causa provável em uma frase (corte numerado a partir de 1).
pub fn explain(cause: &Cause, cut: usize) -> String {
    match cause {
        Cause::Shift { by } => format!(
            "a fala aparece {} s mais {} que a legenda em todo o corte {cut}: position errada no edl.json (ou in e out deslocados juntos)",
            br(format!("{:.2}", by.abs())),
            if *by > 0.0 { "tarde" } else { "cedo" }
        ),
        Cause::Drift { from, to } => format!(
            "o desvio vai de {} s a {} s dentro do corte {cut}: mudança de velocidade na edição ou bruto diferente do usado no corte",
            br(format!("{from:+.2}")),
            br(format!("{to:+.2}"))
        ),
        Cause::Isolated => "palavras soltas: o reconhecimento do vídeo final diferiu ou o pareamento errou; confira ouvindo".into(),
    }
}

/// Correção proposta no `edl.json` (só é aplicada com aprovação).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Fix {
    pub cut: usize,
    pub field: String,
    pub old: f64,
    pub new: f64,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Check {
    pub matched: usize,
    /// Palavras da legenda e da verificação que ficaram sem par.
    pub unmatched_subs: usize,
    pub unmatched_check: usize,
    /// Desvio absoluto do início das palavras casadas.
    pub median: f64,
    pub max: f64,
    pub issues: Vec<Issue>,
    pub fixes: Vec<Fix>,
}

fn median(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}

/// Alinha as palavras da verificação com as da legenda por sequência (texto normalizado:
/// minúsculas, sem pontuação nem acento) e mede o desvio do início dos pares casados.
pub fn check(cuts: &[Cut], built: &Built, check_words: &[Word]) -> Check {
    let norm = crate::compare::normalize;
    let a: Vec<(String, usize)> = built.words.iter().enumerate().map(|(i, w)| (norm(&w.word), i)).filter(|(n, _)| !n.is_empty()).collect();
    let b: Vec<(String, usize)> = check_words.iter().enumerate().map(|(i, w)| (norm(&w.word), i)).filter(|(n, _)| !n.is_empty()).collect();
    let ak: Vec<&str> = a.iter().map(|(n, _)| n.as_str()).collect();
    let bk: Vec<&str> = b.iter().map(|(n, _)| n.as_str()).collect();

    // (índice da palavra da legenda, desvio com sinal = verificação − legenda)
    let mut pairs: Vec<(usize, f64)> = Vec::new();
    for op in capture_diff_slices(Algorithm::Myers, &ak, &bk) {
        let (tag, ra, rb) = op.as_tag_tuple();
        if tag == DiffTag::Equal {
            for (x, y) in ra.zip(rb) {
                let (si, ci) = (a[x].1, b[y].1);
                pairs.push((si, ms(check_words[ci].start - built.words[si].start)));
            }
        }
    }
    let mut abs: Vec<f64> = pairs.iter().map(|(_, d)| d.abs()).collect();
    let max = abs.iter().cloned().fold(0.0, f64::max);
    let med = median(&mut abs);

    // Diagnóstico por corte. O desvio de base (o reconhecimento do vídeo final costuma
    // marcar as palavras alguns centésimos antes ou depois) vem dos cortes sem problema e
    // é descontado do deslocamento proposto.
    let per_cut: Vec<Vec<f64>> = (0..cuts.len()).map(|k| pairs.iter().filter(|(si, _)| built.words[*si].cut == k).map(|(_, d)| *d).collect()).collect();
    let mut base: Vec<f64> = per_cut.iter().filter(|ds| median(&mut ds.to_vec()).abs() <= DESVIO_LISTAR).flatten().copied().collect();
    let base = median(&mut base);
    let mut causes: Vec<Option<Cause>> = vec![None; cuts.len()];
    let mut fixes = Vec::new();
    for (k, cause) in causes.iter_mut().enumerate() {
        let ds = &per_cut[k];
        if !ds.iter().any(|d| d.abs() > DESVIO_LISTAR) {
            continue;
        }
        if ds.len() < MIN_PARES_CORTE {
            *cause = Some(Cause::Isolated);
            continue;
        }
        let m = median(&mut ds.clone());
        let spread = median(&mut ds.iter().map(|d| (d - m).abs()).collect::<Vec<_>>());
        let by = ms(m - base);
        *cause = Some(if by.abs() > DESVIO_LISTAR && spread <= ESPALHAMENTO_MAX {
            if let Some(f) = shift_fix(cuts, k, by) {
                fixes.extend(f);
            }
            Cause::Shift { by }
        } else if spread > ESPALHAMENTO_MAX {
            let n = ds.len();
            let q = (n / 4).max(1);
            Cause::Drift { from: ms(median(&mut ds[..q].to_vec())), to: ms(median(&mut ds[n - q..].to_vec())) }
        } else {
            Cause::Isolated
        });
    }

    // Trechos: palavras com desvio grande, seguidas, no mesmo corte.
    let mut issues: Vec<Issue> = Vec::new();
    let mut last_si: Option<usize> = None;
    for &(si, d) in &pairs {
        if d.abs() <= DESVIO_LISTAR {
            continue;
        }
        let w = &built.words[si];
        // Corte inteiro deslocado: um trecho só por corte.
        let whole = matches!(causes[w.cut], Some(Cause::Shift { .. }));
        let join = matches!((issues.last(), last_si), (Some(is), Some(l)) if is.cut == w.cut && (whole || si <= l + 3));
        if join {
            let is = issues.last_mut().unwrap();
            is.end = w.end;
            is.words += 1;
            if d.abs() > is.worst.abs() {
                is.worst = d;
            }
        } else {
            let cause = causes[w.cut].clone().unwrap_or(Cause::Isolated);
            let why = explain(&cause, w.cut + 1);
            issues.push(Issue { cut: w.cut, start: w.start, end: w.end, words: 1, worst: d, cause, why });
        }
        last_si = Some(si);
    }

    Check {
        matched: pairs.len(),
        unmatched_subs: a.len() - pairs.len(),
        unmatched_check: b.len() - pairs.len(),
        median: ms(med),
        max: ms(max),
        issues,
        fixes,
    }
}

/// Resolução dos tempos do Parakeet (um quadro do modelo): o deslocamento medido tem
/// essa incerteza.
pub const RESOLUCAO: f64 = 0.08;

/// Correção para um corte deslocado em `by` s: mover `position`. Se isso sobrepuser o
/// vizinho por menos de um quadro (incerteza da medição), o corte fica encostado nele. Se
/// sobrepuser mais, mover `in` e `out` juntos (o mesmo trecho do bruto, deslocado).
fn shift_fix(cuts: &[Cut], k: usize, by: f64) -> Option<Vec<Fix>> {
    let c = &cuts[k];
    let want = ms(c.position + by);
    // Limites para o position sem sobrepor os vizinhos.
    let lo = cuts.iter().enumerate().filter(|(j, o)| *j != k && o.position <= c.position).map(|(_, o)| o.final_end()).fold(0.0, f64::max);
    let hi = cuts.iter().enumerate().filter(|(j, o)| *j != k && o.position > c.position).map(|(_, o)| o.position - c.span()).fold(f64::INFINITY, f64::min);
    let pos = ms(want.clamp(lo, hi.max(lo)));
    if (pos - want).abs() <= RESOLUCAO {
        let mut moved = cuts.to_vec();
        moved[k].position = pos;
        if validate(&moved).is_ok() && pos != c.position {
            return Some(vec![Fix { cut: k + 1, field: "position".into(), old: c.position, new: pos }]);
        }
    }
    let (ni, no) = (ms(c.in_ - by), ms(c.out - by));
    (ni >= 0.0).then(|| vec![Fix { cut: k + 1, field: "in".into(), old: c.in_, new: ni }, Fix { cut: k + 1, field: "out".into(), old: c.out, new: no }])
}

/// Aplica as correções aprovadas no `edl.json`, mudando só os campos indicados (o resto do
/// arquivo, inclusive campos extras, fica como está). Guarda uma cópia em `edl.json.bak`.
pub fn apply_fixes(edl: &Path, fixes: &[Fix]) -> Result<PathBuf, String> {
    let text = std::fs::read_to_string(edl).map_err(|e| e.to_string())?;
    let mut v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("edl.json inválido: {e}"))?;
    let arr = v.as_array_mut().ok_or("edl.json inválido: não é uma lista")?;
    for f in fixes {
        if !matches!(f.field.as_str(), "in" | "out" | "position") || !f.new.is_finite() {
            return Err(format!("correção inválida: {}", f.field));
        }
        let obj = arr.get_mut(f.cut.wrapping_sub(1)).and_then(|o| o.as_object_mut()).ok_or(format!("corte {} não existe", f.cut))?;
        let cur = obj.get(&f.field).and_then(|x| x.as_f64());
        if cur.map(|c| (c - f.old).abs() > 0.0005).unwrap_or(true) {
            return Err(format!("o edl.json mudou desde a verificação (corte {}, {}); verifique de novo", f.cut, f.field));
        }
        obj.insert(f.field.clone(), serde_json::json!(f.new));
    }
    // A versão corrigida tem de continuar válida.
    let cuts: Vec<Cut> = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
    validate(&cuts).map_err(|e| e.join("; "))?;
    let bak = PathBuf::from(format!("{}.bak", edl.display()));
    std::fs::copy(edl, &bak).map_err(|e| format!("não foi possível guardar a cópia: {e}"))?;
    let out = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())? + "\n";
    std::fs::write(edl, out).map_err(|e| e.to_string())?;
    Ok(bak)
}

/// Vírgula decimal no texto para a pessoa.
fn br(n: String) -> String {
    n.replace('.', ",")
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

/// `M:SS.cc` para mensagens.
pub fn clock(t: f64) -> String {
    let cs = (t.max(0.0) * 100.0).round() as u64;
    format!("{}:{:02}.{:02}", cs / 6000, cs / 100 % 60, cs % 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cut(file: &str, i: f64, o: f64, p: f64) -> Cut {
        Cut { file: file.into(), in_: i, out: o, position: p }
    }

    /// Bruto com uma palavra por segundo: "p0" em 0–0,8 s, "p1" em 1–1,8 s…
    fn bruto(name: &str, n: usize) -> Transcript {
        Transcript {
            source: name.into(),
            duration: n as f64,
            words: (0..n).map(|i| Word { word: format!("{name}{i}"), start: i as f64, end: i as f64 + 0.8 }).collect(),
            pauses: (0..n).map(|i| Pause { start: i as f64 + 0.8, end: i as f64 + 1.0 }).collect(),
        }
    }

    #[test]
    fn le_o_formato_do_prompt() {
        let c = parse(r#"[{"file": "bruto1.mp4", "in": 12.40, "out": 25.10, "position": 0.00}]"#).unwrap();
        assert_eq!(c, vec![cut("bruto1.mp4", 12.4, 25.1, 0.0)]);
        assert!(parse("[]").is_err());
        assert!(parse(r#"[{"file": "a.mp4", "in": 1}]"#).unwrap_err().contains("edl.json inválido"));
    }

    #[test]
    fn valida_out_maior_que_in_e_sobreposicao() {
        let e = validate(&[cut("a.mp4", 5.0, 5.0, 0.0)]).unwrap_err();
        assert!(e[0].contains("out (5.000) precisa ser maior que in"));
        // Corte 2 começa em 8 s, mas o corte 1 vai até 10 s no vídeo final.
        let e = validate(&[cut("a.mp4", 0.0, 10.0, 0.0), cut("b.mp4", 0.0, 5.0, 8.0)]).unwrap_err();
        assert_eq!(e.len(), 1);
        assert!(e[0].contains("corte 2 (b.mp4) sobrepõe o corte 1 (a.mp4)"), "{}", e[0]);
        // Colados: válido. Lacuna no começo e entre cortes: listadas, em ordem de position.
        let g = validate(&[cut("b.mp4", 0.0, 2.0, 13.0), cut("a.mp4", 0.0, 10.0, 1.0)]).unwrap();
        assert_eq!(g, vec![Gap { start: 0.0, end: 1.0 }, Gap { start: 11.0, end: 13.0 }]);
        assert!(validate(&[cut("a.mp4", 0.0, 10.0, 0.0), cut("b.mp4", 0.0, 5.0, 10.0)]).unwrap().is_empty());
    }

    #[test]
    fn palavra_entra_pelo_inicio_e_o_fim_e_cortado_em_out() {
        let t = bruto("a", 10);
        // [2,5) a partir de 0: entram a2, a3, a4 (a5 começa em 5 = out, fica de fora).
        let (w, p) = map_cut(&cut("a.mp4", 2.0, 5.0, 0.0), &t);
        assert_eq!(w.iter().map(|x| x.word.as_str()).collect::<Vec<_>>(), ["a2", "a3", "a4"]);
        assert_eq!((w[0].start, w[0].end), (0.0, 0.8));
        // Fim cortado em out: [2, 4.5) → a4 começa em 4 e terminaria em 4,8.
        let (w, _) = map_cut(&cut("a.mp4", 2.0, 4.5, 10.0), &t);
        assert_eq!((w[2].start, w[2].end), (12.0, 12.5));
        // Pausas recortadas ao corte e deslocadas.
        assert!(p.iter().all(|x| x.start >= 0.0 && x.end <= 3.0));
    }

    #[test]
    fn nenhum_bloco_atravessa_um_corte() {
        let a = bruto("a", 20);
        let b = bruto("b", 20);
        let cuts = [cut("a.mp4", 0.0, 3.0, 0.0), cut("b.mp4", 5.0, 8.0, 3.0)];
        let built = build(&cuts, &[a, b]);
        // Cada bloco só tem palavras de um corte.
        for c in &built.cues {
            assert!(c.text.split(' ').all(|w| w.starts_with('a')) || c.text.split(' ').all(|w| w.starts_with('b')), "{}", c.text);
        }
        let last_a = built.cues.iter().rfind(|c| c.text.starts_with('a')).unwrap();
        let first_b = built.cues.iter().find(|c| c.text.starts_with('b')).unwrap();
        assert!(last_a.end <= 3.0 && last_a.end <= first_b.start - subs::INTERVALO_MIN + 1e-9);
        assert_eq!(first_b.start, 3.0);
    }

    #[test]
    fn falta_do_json_do_bruto_e_erro_claro() {
        let dir = std::env::temp_dir().join(format!("ouveai_edl_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let edl = dir.join("edl.json");
        let e = load_sources(&edl, &[cut("bruto1.mp4", 0.0, 1.0, 0.0), cut("bruto1.mp4", 2.0, 3.0, 1.0)]).unwrap_err();
        assert_eq!(e, vec!["falta bruto1.json: transcreva bruto1.mp4 antes (o .json fica ao lado dele)".to_string()]);
        std::fs::remove_dir_all(&dir).ok();
    }

    fn final_words(built: &Built, shift: impl Fn(&FinalWord) -> f64) -> Vec<Word> {
        built.words.iter().map(|w| Word { word: w.word.to_uppercase() + ".", start: w.start + shift(w), end: w.end + shift(w) }).collect()
    }

    #[test]
    fn verificacao_sem_desvio() {
        let cuts = [cut("a.mp4", 0.0, 10.0, 0.0), cut("b.mp4", 0.0, 10.0, 12.0)];
        let built = build(&cuts, &[bruto("a", 10), bruto("b", 10)]);
        let r = check(&cuts, &built, &final_words(&built, |_| 0.05));
        assert_eq!((r.matched, r.unmatched_subs, r.unmatched_check), (20, 0, 0));
        assert_eq!((r.median, r.max), (0.05, 0.05));
        assert!(r.issues.is_empty() && r.fixes.is_empty());
    }

    #[test]
    fn corte_deslocado_propoe_mudar_position() {
        let cuts = [cut("a.mp4", 0.0, 10.0, 0.0), cut("b.mp4", 0.0, 10.0, 12.0)];
        let built = build(&cuts, &[bruto("a", 10), bruto("b", 10)]);
        // No vídeo final, o corte 2 começa 0,4 s mais tarde do que diz o edl.json.
        let r = check(&cuts, &built, &final_words(&built, |w| if w.cut == 1 { 0.4 } else { 0.0 }));
        assert_eq!(r.max, 0.4);
        assert_eq!(r.issues.len(), 1);
        assert_eq!((r.issues[0].cut, r.issues[0].words, &r.issues[0].cause), (1, 10, &Cause::Shift { by: 0.4 }));
        assert_eq!(r.fixes, vec![Fix { cut: 2, field: "position".into(), old: 12.0, new: 12.4 }]);
    }

    #[test]
    fn deslocamento_que_sobreporia_vira_mudanca_de_in_e_out() {
        // Cortes colados: adiantar o corte 2 o faria sobrepor o 1.
        let cuts = [cut("a.mp4", 0.0, 10.0, 0.0), cut("b.mp4", 2.0, 12.0, 10.0)];
        let built = build(&cuts, &[bruto("a", 10), bruto("b", 14)]);
        let r = check(&cuts, &built, &final_words(&built, |w| if w.cut == 1 { -0.5 } else { 0.0 }));
        assert_eq!(
            r.fixes,
            vec![Fix { cut: 2, field: "in".into(), old: 2.0, new: 2.5 }, Fix { cut: 2, field: "out".into(), old: 12.0, new: 12.5 }]
        );
    }

    #[test]
    fn deslocamento_um_pouco_alem_do_vizinho_encosta_nele() {
        // O corte 2 deveria ir para 12,44, mas o corte 3 começa em 22,4: dentro da incerteza
        // de um quadro, o corte 2 fica encostado (12,4).
        let cuts = [cut("a.mp4", 0.0, 10.0, 0.0), cut("b.mp4", 0.0, 10.0, 12.0), cut("a.mp4", 0.0, 5.0, 22.4)];
        let built = build(&cuts, &[bruto("a", 10), bruto("b", 10), bruto("a", 10)]);
        let r = check(&cuts, &built, &final_words(&built, |w| if w.cut == 1 { 0.44 } else { 0.0 }));
        assert_eq!(r.fixes, vec![Fix { cut: 2, field: "position".into(), old: 12.0, new: 12.4 }]);
    }

    #[test]
    fn desvio_que_cresce_e_deslize_e_nao_tem_correcao_automatica() {
        let cuts = [cut("a.mp4", 0.0, 20.0, 0.0)];
        let built = build(&cuts, &[bruto("a", 20)]);
        let r = check(&cuts, &built, &final_words(&built, |w| w.start * 0.05));
        assert!(matches!(r.issues[0].cause, Cause::Drift { from, to } if from < 0.2 && to > 0.7));
        assert!(r.fixes.is_empty());
    }

    #[test]
    fn palavra_isolada_e_sem_par() {
        let cuts = [cut("a.mp4", 0.0, 10.0, 0.0)];
        let built = build(&cuts, &[bruto("a", 10)]);
        let mut fw = final_words(&built, |w| if w.word == "a5" { 0.6 } else { 0.0 });
        fw.remove(2); // a2 não aparece na verificação
        fw.push(Word { word: "extra".into(), start: 9.9, end: 10.0 });
        let r = check(&cuts, &built, &fw);
        assert_eq!((r.matched, r.unmatched_subs, r.unmatched_check), (9, 1, 1));
        assert_eq!(r.issues.len(), 1);
        assert_eq!((r.issues[0].worst, &r.issues[0].cause), (0.6, &Cause::Isolated));
        assert!(r.fixes.is_empty());
    }

    #[test]
    fn aplicar_correcao_muda_so_o_campo_e_guarda_copia() {
        let dir = std::env::temp_dir().join(format!("ouveai_edl_fix_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let edl = dir.join("edl.json");
        let orig = r#"[{"file": "a.mp4", "in": 0, "out": 10, "position": 0, "nota": "abertura"}, {"file": "b.mp4", "in": 0, "out": 10, "position": 12}]"#;
        std::fs::write(&edl, orig).unwrap();
        let fix = Fix { cut: 2, field: "position".into(), old: 12.0, new: 12.4 };
        let bak = apply_fixes(&edl, std::slice::from_ref(&fix)).unwrap();
        assert_eq!(std::fs::read_to_string(&bak).unwrap(), orig);
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&edl).unwrap()).unwrap();
        assert_eq!(v[1]["position"], 12.4);
        assert_eq!(v[0]["nota"], "abertura");
        // De novo: o valor antigo não confere mais.
        assert!(apply_fixes(&edl, &[fix]).unwrap_err().contains("mudou desde a verificação"));
        // Correção que criaria sobreposição é recusada.
        let bad = Fix { cut: 2, field: "position".into(), old: 12.4, new: 5.0 };
        assert!(apply_fixes(&edl, &[bad]).unwrap_err().contains("sobrepõe"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
