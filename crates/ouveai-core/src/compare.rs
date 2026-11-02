//! Parte 4: comparação Parakeet × Whisper. Alinha as duas sequências de palavras
//! (texto normalizado: minúsculas, sem acento e sem pontuação) e devolve só os
//! trechos que divergem. Ao aplicar uma escolha, o texto vem do Whisper e os tempos
//! continuam os do Parakeet: o Whisper não mede o tempo de cada palavra.

use crate::words::Word;
use similar::{Algorithm, DiffTag, capture_diff_slices};

/// Um trecho em que os dois modelos discordam.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Divergence {
    /// Palavras do Parakeet `[from, to)` (índices na lista recebida).
    pub from: usize,
    pub to: usize,
    pub start: f64,
    pub end: f64,
    pub parakeet: String,
    pub whisper: String,
}

/// Minúsculas, sem acento e sem pontuação ("Reunião," → "reuniao").
pub fn normalize(w: &str) -> String {
    w.chars()
        .flat_map(char::to_lowercase)
        .filter_map(|c| {
            let base = match c {
                'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
                'é' | 'è' | 'ê' | 'ë' => 'e',
                'í' | 'ì' | 'î' | 'ï' => 'i',
                'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
                'ú' | 'ù' | 'û' | 'ü' => 'u',
                'ç' => 'c',
                'ñ' => 'n',
                c => c,
            };
            base.is_alphanumeric().then_some(base)
        })
        .collect()
}

/// Chave de alinhamento: `normalize` mais as formas coloquiais que o Whisper costuma
/// escrever por extenso ("tá" e "está" contam como iguais).
fn key(w: &str) -> String {
    let n = normalize(w);
    match n.as_str() {
        "ta" => "esta".into(),
        "tava" => "estava".into(),
        "tavam" => "estavam".into(),
        "to" => "estou".into(),
        "tamo" | "tamos" => "estamos".into(),
        "pra" => "para".into(),
        "ce" => "voce".into(),
        "ces" => "voces".into(),
        _ => n,
    }
}

/// Muletas que o Whisper costuma omitir. Um trecho cuja diferença é só de muletas não
/// é mostrado: o texto do Parakeet já é a fala literal.
const FILLERS: &[&str] = &["ne", "po", "cara", "ali", "tal", "e", "a", "o", "entao", "assim", "tipo", "ai", "dai", "ah", "eh", "hum", "uh", "hm", "mano", "porra"];

fn only_fillers_differ(p: &[&str], w: &[&str]) -> bool {
    let kept = |v: &[&str]| v.iter().filter(|x| !FILLERS.contains(x)).map(|x| x.to_string()).collect::<Vec<_>>();
    kept(p) == kept(w)
}

/// Separa o texto do Whisper em palavras.
pub fn split_words(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_string).collect()
}

/// Trechos divergentes. Diferenças separadas por uma só palavra igual viram um trecho
/// só (mais fácil de ler). Uma palavra que só o Whisper ouviu leva junto a palavra
/// vizinha do Parakeet, para o trecho ter tempo.
pub fn diverge(parakeet: &[Word], whisper: &[String]) -> Vec<Divergence> {
    // Palavras que viram vazio (só pontuação) não entram no alinhamento.
    let p: Vec<(String, usize)> = parakeet.iter().enumerate().map(|(i, w)| (key(&w.word), i)).filter(|(n, _)| !n.is_empty()).collect();
    let w: Vec<(String, usize)> = whisper.iter().enumerate().map(|(i, w)| (key(w), i)).filter(|(n, _)| !n.is_empty()).collect();
    if p.is_empty() {
        return Vec::new();
    }
    let pk: Vec<&str> = p.iter().map(|x| x.0.as_str()).collect();
    let wk: Vec<&str> = w.iter().map(|x| x.0.as_str()).collect();

    // Trechos em índices das listas normalizadas.
    let mut hunks: Vec<(std::ops::Range<usize>, std::ops::Range<usize>)> = Vec::new();
    let mut last_equal = usize::MAX;
    for op in capture_diff_slices(Algorithm::Myers, &pk, &wk) {
        let (tag, a, b) = op.as_tag_tuple();
        if tag == DiffTag::Equal {
            last_equal = a.len();
            continue;
        }
        match hunks.last_mut() {
            Some((pa, pb)) if last_equal <= 1 => {
                pa.end = a.end.max(pa.end);
                pb.end = b.end.max(pb.end);
            }
            _ => hunks.push((a, b)),
        }
        last_equal = 0;
    }

    hunks
        .into_iter()
        .filter(|(a, b)| !only_fillers_differ(&pk[a.clone()], &wk[b.clone()]))
        .map(|(mut a, mut b)| {
            if a.is_empty() {
                // Só o Whisper tem palavras aqui: leva a vizinha (igual nos dois).
                if a.start > 0 {
                    a.start -= 1;
                    b.start -= 1;
                } else {
                    a.end += 1;
                    b.end += 1;
                }
            }
            // De volta aos índices originais (incluindo a pontuação solta do meio).
            let from = p[a.start].1;
            let to = p[a.end - 1].1 + 1;
            let wtext = if b.is_empty() {
                String::new()
            } else {
                whisper[w[b.start].1..=w[b.end - 1].1].join(" ")
            };
            Divergence {
                from,
                to,
                start: parakeet[from].start,
                end: parakeet[to - 1].end,
                parakeet: parakeet[from..to].iter().map(|x| x.word.as_str()).collect::<Vec<_>>().join(" "),
                whisper: wtext,
            }
        })
        .collect()
}

/// Aplica as escolhas do Whisper: troca as palavras de cada trecho pelas do Whisper,
/// nos tempos medidos do Parakeet (ver `measured`).
/// `chosen` não pode ter trechos sobrepostos.
pub fn apply(words: &[Word], chosen: &[Divergence]) -> Vec<Word> {
    let mut chosen: Vec<&Divergence> = chosen.iter().collect();
    chosen.sort_by_key(|d| d.from);
    let mut out = Vec::with_capacity(words.len());
    let mut i = 0;
    for d in chosen {
        if d.from < i || d.to > words.len() || d.from >= d.to {
            continue; // sobreposto ou inválido
        }
        out.extend_from_slice(&words[i..d.from]);
        out.extend(measured(&words[d.from..d.to], &split_words(&d.whisper)));
        i = d.to;
    }
    out.extend_from_slice(&words[i..]);
    out
}

/// Põe o texto do Whisper nos tempos medidos das palavras do Parakeet que ele substitui,
/// sem inventar tempo (regra do prompt: tempo sempre medido, nunca interpolado).
/// - Mesma quantidade de palavras: cada uma fica com o tempo da que substitui.
/// - Quantidade diferente: o trecho vira uma unidade só ("começar logo"), com o início da
///   primeira e o fim da última palavra substituída. A legenda nunca quebra uma unidade.
/// - Nada do Whisper: as palavras somem.
fn measured(old: &[Word], ws: &[String]) -> Vec<Word> {
    if ws.is_empty() || old.is_empty() {
        return Vec::new();
    }
    if ws.len() == old.len() {
        return old.iter().zip(ws).map(|(o, w)| Word { word: w.clone(), start: o.start, end: o.end }).collect();
    }
    vec![Word { word: ws.join(" "), start: old[0].start, end: old[old.len() - 1].end }]
}

// ---------- execução do Whisper ----------

/// Janela padrão do Whisper, em segundos. Medido na fase 7: com 30 s o Whisper entra em
/// laço e engole frases com frequência; com 20 s quase não; com 15 s volta a engolir.
pub const WHISPER_CHUNK: f64 = 20.0;

/// Uma janela enviada ao Whisper. `words` é `None` quando a janela foi pulada
/// (nenhuma palavra do Parakeet nela).
#[derive(Clone, Debug, serde::Serialize)]
pub struct Window {
    pub start: f64,
    pub end: f64,
    pub words: Option<Vec<String>>,
}

/// Resultado de uma passada do Whisper.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Pass {
    pub windows: Vec<Window>,
    /// Tempo do reconhecedor.
    pub secs: f64,
}

impl Pass {
    pub fn skipped(&self) -> usize {
        self.windows.iter().filter(|w| w.words.is_none()).count()
    }
    pub fn word_count(&self) -> usize {
        self.windows.iter().filter_map(|w| w.words.as_ref()).map(Vec::len).sum()
    }
}

/// Divergências de uma passada inteira, mais as janelas em que o Whisper falhou.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Outcome {
    pub divergences: Vec<Divergence>,
    /// `(início, fim)` das janelas descartadas: o Whisper repetiu frases em laço,
    /// inventou texto ou engoliu boa parte da fala.
    pub unreliable: Vec<(f64, f64)>,
}

/// Mesma sequência de 1 a 8 palavras repetida 3 vezes seguidas, ou de 4 a 12 palavras
/// repetida 2 vezes: o laço típico do Whisper.
fn has_loop(ws: &[String]) -> bool {
    let k: Vec<String> = ws.iter().map(|w| normalize(w)).filter(|w| !w.is_empty()).collect();
    let rep = |n: usize, times: usize| (0..k.len().saturating_sub(n * times - 1)).any(|i| (1..times).all(|t| k[i..i + n] == k[i + t * n..i + (t + 1) * n]));
    (1..=8).any(|n| rep(n, 3)) || (4..=12).any(|n| rep(n, 2))
}

/// O Whisper falhou nesta janela? Laço, texto bem maior que o do Parakeet (invenção) ou
/// bem menor (engoliu fala).
fn unreliable(parakeet: &[Word], ws: &[String]) -> bool {
    let p = parakeet.iter().filter(|w| !normalize(&w.word).is_empty()).count();
    let w = ws.iter().filter(|w| !normalize(w).is_empty()).count();
    has_loop(ws) || w > p * 3 / 2 + 5 || (p >= 8 && w < p / 2)
}

/// Alinha cada janela com as palavras do Parakeet que caem nela (pelo meio da palavra) e
/// junta as divergências, com índices na lista inteira.
pub fn diverge_pass(parakeet: &[Word], pass: &Pass) -> Outcome {
    let mid = |w: &Word| (w.start + w.end) / 2.0;
    let mut out = Outcome::default();
    for win in &pass.windows {
        let Some(ws) = &win.words else { continue };
        let i0 = parakeet.partition_point(|w| mid(w) < win.start);
        let i1 = parakeet.partition_point(|w| mid(w) < win.end);
        diverge_into(&mut out, &parakeet[i0..i1], ws, i0, (win.start, win.end));
    }
    out
}

/// Um pedaço já recortado (ex.: um segmento da reunião): as palavras do Parakeet, a
/// posição da primeira na lista da sessão e o texto do Whisper (`None` = não comparado).
pub struct Piece<'a> {
    pub words: &'a [Word],
    pub offset: usize,
    pub whisper: Option<Vec<String>>,
}

/// Como `diverge_pass`, mas com os pedaços já recortados (segmentos de canais diferentes
/// podem se sobrepor no tempo, então não dá para fatiar por tempo).
pub fn diverge_pieces(pieces: &[Piece]) -> Outcome {
    let mut out = Outcome::default();
    for p in pieces {
        let (Some(ws), Some(first), Some(last)) = (&p.whisper, p.words.first(), p.words.last()) else { continue };
        diverge_into(&mut out, p.words, ws, p.offset, (first.start, last.end));
    }
    out
}

fn diverge_into(out: &mut Outcome, slice: &[Word], ws: &[String], offset: usize, span: (f64, f64)) {
    if slice.is_empty() {
        return;
    }
    if unreliable(slice, ws) {
        out.unreliable.push(span);
        return;
    }
    out.divergences.extend(diverge(slice, ws).into_iter().map(|mut d| {
        d.from += offset;
        d.to += offset;
        d
    }));
}

/// As palavras do Whisper de um trecho escolhido, nos tempos medidos das palavras do
/// Parakeet que elas substituem (ver `measured`).
pub fn whisper_words(words: &[Word], d: &Divergence) -> Vec<Word> {
    measured(&words[d.from..d.to], &split_words(&d.whisper))
}

/// Trechos de até `max` s (no máximo 30, o limite do Whisper) para o Whisper, cortados no
/// maior silêncio medido entre 3/4 de `max` e `max`.
pub fn file_ranges(samples: &[f32], pauses: &[crate::words::Pause], max: f64) -> Vec<(f64, f64)> {
    let plan = crate::media::ChunkPlan::with_max(max.min(crate::asr::WHISPER_MAX_SECONDS));
    crate::media::plan_chunks_with(samples, crate::asr::SAMPLE_RATE as u32, pauses, plan).into_iter().map(|c| (c.start, c.end)).collect()
}

/// Converte o arquivo (FFmpeg) e devolve as amostras 16 kHz mono. O WAV temporário é apagado.
pub fn file_samples(ff: &std::path::Path, input: &std::path::Path, cancel: &std::sync::atomic::AtomicBool) -> Result<Vec<f32>, crate::transcribe::Error> {
    use crate::transcribe::Error;
    let wav = crate::media::to_wav(ff, input, cancel)?;
    let wave = sherpa_onnx::Wave::read(&wav.path.to_string_lossy()).ok_or_else(|| Error::Invalid("não foi possível ler o áudio convertido".into()))?;
    if wave.sample_rate() != crate::asr::SAMPLE_RATE {
        return Err(Error::Invalid(format!("taxa inesperada: {} Hz", wave.sample_rate())));
    }
    Ok(wave.samples().to_vec())
}

/// Roda o Whisper em cada trecho `(início, fim)` em segundos. Trecho sem nenhuma palavra
/// do Parakeet é pulado: no silêncio e no ruído o Whisper costuma inventar frases
/// ("Obrigado.", "Legendas pela comunidade…").
pub fn whisper_pass(
    wh: &crate::asr::Whisper,
    samples: &[f32],
    ranges: &[(f64, f64)],
    parakeet: &[Word],
    cancel: &std::sync::atomic::AtomicBool,
    on: &mut dyn FnMut(usize, usize),
) -> Result<Pass, String> {
    let sr = crate::asr::SAMPLE_RATE as f64;
    let t0 = std::time::Instant::now();
    let mut windows = Vec::with_capacity(ranges.len());
    for (i, &(a, b)) in ranges.iter().enumerate() {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("cancelado".into());
        }
        on(i, ranges.len());
        let mid = |w: &&Word| (w.start + w.end) / 2.0;
        let words = if parakeet.iter().any(|w| (a..b).contains(&mid(&w)) && !normalize(&w.word).is_empty()) {
            let (i0, i1) = (((a * sr) as usize).min(samples.len()), ((b * sr) as usize).min(samples.len()));
            Some(split_words(&wh.recognize(&samples[i0..i1])?))
        } else {
            None
        };
        windows.push(Window { start: a, end: b, words });
    }
    on(ranges.len(), ranges.len());
    Ok(Pass { windows, secs: t0.elapsed().as_secs_f64() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws(s: &str) -> Vec<Word> {
        s.split_whitespace().enumerate().map(|(i, w)| Word { word: w.into(), start: i as f64, end: i as f64 + 0.8 }).collect()
    }
    fn wh(s: &str) -> Vec<String> {
        split_words(s)
    }

    #[test]
    fn normaliza_acento_caixa_e_pontuacao() {
        assert_eq!(normalize("Reunião,"), "reuniao");
        assert_eq!(normalize("AÇÃO!"), "acao");
        assert_eq!(normalize("—"), "");
        assert_eq!(normalize("R$10"), "r10");
    }

    #[test]
    fn iguais_a_menos_de_acento_e_pontuacao_nao_divergem() {
        let p = ws("vamos começar a reuniao , pessoal");
        assert!(diverge(&p, &wh("Vamos começar a reunião, pessoal.")).is_empty());
    }

    #[test]
    fn troca_vira_um_trecho_com_os_tempos_do_parakeet() {
        let p = ws("o mike soft chegou ontem");
        let d = diverge(&p, &wh("O Microsoft chegou ontem."));
        assert_eq!(d.len(), 1);
        assert_eq!((d[0].from, d[0].to), (1, 3));
        assert_eq!(d[0].parakeet, "mike soft");
        assert_eq!(d[0].whisper, "Microsoft");
        assert_eq!((d[0].start, d[0].end), (1.0, 2.8));
    }

    #[test]
    fn diferencas_separadas_por_uma_palavra_se_juntam() {
        let p = ws("a reunion de hoje but foi boa");
        let d = diverge(&p, &wh("a reunião de hoje mas foi boa"));
        // "reunion"→"reunião" e "but"→"mas" ficam separados por "de hoje" (2 palavras).
        assert_eq!(d.len(), 2);
        let d = diverge(&ws("a reunion de but foi"), &wh("a reunião de mas foi"));
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].parakeet, "reunion de but");
        assert_eq!(d[0].whisper, "reunião de mas");
    }

    #[test]
    fn palavra_so_do_whisper_leva_a_vizinha() {
        let p = ws("vamos começar agora");
        let d = diverge(&p, &wh("vamos começar logo agora"));
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].parakeet, "começar");
        assert_eq!(d[0].whisper, "começar logo");
        // No começo, leva a palavra seguinte.
        let d = diverge(&p, &wh("hoje vamos começar agora"));
        assert_eq!((d[0].parakeet.as_str(), d[0].whisper.as_str()), ("vamos", "hoje vamos"));
    }

    #[test]
    fn palavra_que_o_whisper_nao_ouviu() {
        let d = diverge(&ws("isso isso aqui"), &wh("isso aqui"));
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].whisper, "");
    }

    #[test]
    fn formas_coloquiais_e_muletas_nao_divergem() {
        let p = ws("ele tava lá , né , e cara pra mim tá bom");
        assert!(diverge(&p, &wh("Ele estava lá e para mim está bom.")).is_empty());
        // Muleta a mais no Whisper também não conta.
        assert!(diverge(&ws("mas foi"), &wh("Mas, pô, foi")).is_empty());
        // Muleta junto de uma troca de verdade: o trecho aparece.
        assert_eq!(diverge(&ws("né o mike soft"), &wh("o Microsoft")).len(), 1);
    }

    #[test]
    fn laco_do_whisper_e_detectado() {
        assert!(has_loop(&wh("ele tem um ele tem um ele tem um")));
        assert!(has_loop(&wh("a gente vai ver isso depois a gente vai ver isso depois")));
        assert!(!has_loop(&wh("muitos e muitos dias de chuva")));
        assert!(!has_loop(&wh("uma frase normal sem repetição nenhuma aqui")));
    }

    fn win(start: f64, end: f64, text: Option<&str>) -> Window {
        Window { start, end, words: text.map(wh) }
    }

    #[test]
    fn passada_por_janela_ajusta_indices_e_descarta_janela_ruim() {
        // 0..4 na janela 1 (0–5 s), 5..9 na janela 2 (5–10 s), 10..14 na janela 3.
        let p = ws("a casa é azul e o carro é verde mas o dia foi bom hoje");
        let pass = Pass {
            windows: vec![
                win(0.0, 5.0, Some("a casa é azul e")),
                win(5.0, 10.0, Some("o barco é verde mas")),
                win(10.0, 15.0, Some("tudo tudo tudo tudo tudo tudo")),
            ],
            secs: 0.0,
        };
        let o = diverge_pass(&p, &pass);
        assert_eq!(o.divergences.len(), 1);
        assert_eq!((o.divergences[0].from, o.divergences[0].to), (6, 7));
        assert_eq!(o.divergences[0].parakeet, "carro");
        assert_eq!(o.unreliable, vec![(10.0, 15.0)]);
    }

    #[test]
    fn pedacos_usam_o_deslocamento_de_cada_um() {
        let a = ws("bom dia pessoal");
        let b = ws("o mike soft chegou");
        let o = diverge_pieces(&[
            Piece { words: &a, offset: 0, whisper: Some(wh("bom dia pessoal")) },
            Piece { words: &b, offset: 3, whisper: Some(wh("o Microsoft chegou")) },
            Piece { words: &a, offset: 7, whisper: None },
        ]);
        assert_eq!(o.divergences.len(), 1);
        assert_eq!((o.divergences[0].from, o.divergences[0].to), (4, 6));
        let all: Vec<Word> = a.iter().chain(b.iter()).cloned().collect();
        let nw = whisper_words(&all, &o.divergences[0]);
        assert_eq!((nw.len(), nw[0].word.as_str(), nw[0].start, nw[0].end), (1, "Microsoft", b[1].start, b[2].end));
    }

    #[test]
    fn janela_pulada_nao_gera_divergencia() {
        let p = ws("uma duas três");
        let pass = Pass { windows: vec![win(0.0, 5.0, None)], secs: 0.0 };
        let o = diverge_pass(&p, &pass);
        assert!(o.divergences.is_empty() && o.unreliable.is_empty());
    }

    #[test]
    fn aplicar_mantem_o_intervalo_e_a_ordem() {
        let p = ws("o mike soft chegou ontem");
        let d = diverge(&p, &wh("O Microsoft chegou ontem."));
        let out = apply(&p, &d);
        let texto: Vec<&str> = out.iter().map(|w| w.word.as_str()).collect();
        assert_eq!(texto, ["o", "Microsoft", "chegou", "ontem"]);
        assert_eq!((out[1].start, out[1].end), (1.0, 2.8));
        assert!(out.windows(2).all(|x| x[0].end <= x[1].start + 1e-9));

        // Duas palavras no lugar de uma: viram uma unidade com o tempo medido, sem fronteira inventada.
        let p = ws("vamos começar agora");
        let d = diverge(&p, &wh("vamos começar logo agora"));
        let out = apply(&p, &d);
        assert_eq!(out.len(), 3);
        assert_eq!((out[1].word.as_str(), out[1].start, out[1].end), ("começar logo", p[1].start, p[1].end));

        // Mesma quantidade: cada palavra fica com o tempo da que substitui.
        let p = ws("a reunion de but foi");
        let out = apply(&p, &diverge(&p, &wh("a reunião de mas foi")));
        assert!(out.iter().zip(&p).all(|(a, b)| a.start == b.start && a.end == b.end));
        assert_eq!(out[1].word, "reunião");

        // Escolher "nada" do Whisper apaga a palavra.
        let p = ws("isso isso aqui");
        let out = apply(&p, &diverge(&p, &wh("isso aqui")));
        assert_eq!(out.len(), 2);
    }
}
