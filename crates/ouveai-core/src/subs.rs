//! Segmentação de legenda (função única: arquivo agora, Parte 5 depois) e `.srt`.

use crate::words::{Pause, Word};

// Configuração (prompt.txt). Em segundos ou caracteres.
pub const MAX_CHARS: usize = 36;
pub const PAUSA_QUEBRA: f64 = 0.45;
pub const VIRGULA_MIN_CHARS: usize = 20;
pub const SAIDA_APOS_FIM: f64 = 0.1;
pub const MIN_DURACAO: f64 = 1.0;
pub const INTERVALO_MIN: f64 = 0.08;

#[derive(Clone, Debug, PartialEq)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Silêncio medido entre duas palavras: a parte das pausas dentro de [a, b].
/// Nunca a diferença entre os inícios (regra 3).
pub fn silence_between(pauses: &[Pause], a: f64, b: f64) -> f64 {
    pauses.iter().map(|p| (p.end.min(b) - p.start.max(a)).max(0.0)).sum()
}

fn ends_sentence(w: &str) -> bool {
    w.trim_end_matches(['"', '\'', ')', '»', '”']).ends_with(['.', '?', '!', '…'])
}

/// Agrupa as palavras em blocos de uma linha seguindo as regras de quebra.
pub fn segment(words: &[Word], pauses: &[Pause]) -> Vec<Cue> {
    let mut groups: Vec<Vec<&Word>> = Vec::new();
    let mut cur: Vec<&Word> = Vec::new();
    let mut chars = 0usize;

    for w in words.iter().filter(|w| !w.word.trim().is_empty()) {
        if let Some(prev) = cur.last() {
            let len = w.word.chars().count();
            let brk = ends_sentence(&prev.word)
                || (prev.word.ends_with([',', ';', ':']) && chars >= VIRGULA_MIN_CHARS)
                || silence_between(pauses, prev.end, w.start) >= PAUSA_QUEBRA
                || chars + 1 + len > MAX_CHARS;
            if brk {
                groups.push(std::mem::take(&mut cur));
                chars = 0;
            }
        }
        chars += if cur.is_empty() { 0 } else { 1 } + w.word.chars().count();
        cur.push(w);
    }
    if !cur.is_empty() {
        groups.push(cur);
    }

    let starts: Vec<f64> = groups.iter().map(|g| g[0].start).collect();
    groups
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let start = g[0].start;
            let mut end = g.last().unwrap().end + SAIDA_APOS_FIM;
            if end - start < MIN_DURACAO {
                end = start + MIN_DURACAO;
            }
            if let Some(next) = starts.get(i + 1) {
                end = end.min(next - INTERVALO_MIN);
            }
            // Blocos colados demais: mantém pelo menos 1 ms de duração.
            let end = end.max(start + 0.001);
            Cue { start, end, text: g.iter().map(|w| w.word.as_str()).collect::<Vec<_>>().join(" ") }
        })
        .collect()
}

/// `HH:MM:SS,mmm`
pub fn srt_time(t: f64) -> String {
    let ms = (t.max(0.0) * 1000.0).round() as u64;
    format!("{:02}:{:02}:{:02},{:03}", ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)
}

pub fn to_srt(cues: &[Cue]) -> String {
    let mut s = String::new();
    for (i, c) in cues.iter().enumerate() {
        s.push_str(&format!("{}\r\n{} --> {}\r\n{}\r\n\r\n", i + 1, srt_time(c.start), srt_time(c.end), c.text));
    }
    s
}

/// Texto corrido: palavras separadas por espaço; parágrafo novo quando uma
/// frase termina e vem um silêncio medido de 2 s ou mais.
pub fn to_txt(words: &[Word], pauses: &[Pause]) -> String {
    let mut s = String::new();
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            let prev = &words[i - 1];
            let para = ends_sentence(&prev.word) && silence_between(pauses, prev.end, w.start) >= 2.0;
            s.push_str(if para { "\r\n\r\n" } else { " " });
        }
        s.push_str(&w.word);
    }
    s.push_str("\r\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(word: &str, start: f64, end: f64) -> Word {
        Word { word: word.into(), start, end }
    }

    fn texts(c: &[Cue]) -> Vec<&str> {
        c.iter().map(|c| c.text.as_str()).collect()
    }

    #[test]
    fn quebra_em_pontuacao_final() {
        let c = segment(&[w("Oi.", 0.0, 0.3), w("Tudo", 0.4, 0.6), w("bem?", 0.6, 0.9), w("Sim", 1.0, 1.2)], &[]);
        assert_eq!(texts(&c), ["Oi.", "Tudo bem?", "Sim"]);
    }

    #[test]
    fn virgula_so_com_bloco_longo() {
        let c = segment(&[w("Bom,", 0.0, 0.2), w("vamos", 0.3, 0.5)], &[]);
        assert_eq!(texts(&c), ["Bom, vamos"]);
        let c = segment(&[w("Primeiramente", 0.0, 0.5), w("pessoal,", 0.5, 0.9), w("vamos", 1.0, 1.2)], &[]);
        assert_eq!(texts(&c), ["Primeiramente pessoal,", "vamos"]);
    }

    #[test]
    fn quebra_em_pausa_medida_e_nao_na_diferenca_de_inicios() {
        let words = [w("antes", 0.0, 0.3), w("depois", 2.0, 2.3)];
        // Sem silêncio medido, a distância entre as palavras não quebra.
        assert_eq!(segment(&words, &[]).len(), 1);
        assert_eq!(segment(&words, &[Pause { start: 0.4, end: 1.9 }]).len(), 2);
        // Silêncio curto (< 0,45 s) não quebra.
        assert_eq!(segment(&words, &[Pause { start: 0.5, end: 0.9 }]).len(), 1);
    }

    #[test]
    fn limite_de_caracteres_quebra_na_palavra_anterior() {
        let ws: Vec<Word> = "um dois tres quatro cinco seis sete oito nove".split(' ').enumerate().map(|(i, s)| w(s, i as f64 * 0.3, i as f64 * 0.3 + 0.2)).collect();
        let c = segment(&ws, &[]);
        assert!(c.iter().all(|c| c.text.chars().count() <= MAX_CHARS));
        assert_eq!(texts(&c), ["um dois tres quatro cinco seis sete", "oito nove"]);
    }

    #[test]
    fn duracao_minima_e_intervalo() {
        // Bloco curto é esticado até MIN_DURACAO...
        let c = segment(&[w("Oi.", 0.0, 0.2), w("Tchau.", 5.0, 6.5)], &[]);
        assert!((c[0].end - 1.0).abs() < 1e-9);
        assert!((c[1].end - 6.6).abs() < 1e-9); // fim + SAIDA_APOS_FIM
        // ...mas respeita INTERVALO_MIN antes do próximo.
        let c = segment(&[w("Oi.", 0.0, 0.2), w("Tchau.", 0.6, 0.9)], &[]);
        assert!((c[0].end - (0.6 - INTERVALO_MIN)).abs() < 1e-9);
    }

    #[test]
    fn formato_srt() {
        assert_eq!(srt_time(3725.0456), "01:02:05,046");
        let s = to_srt(&[Cue { start: 0.0, end: 1.0, text: "Olá".into() }]);
        assert_eq!(s, "1\r\n00:00:00,000 --> 00:00:01,000\r\nOlá\r\n\r\n");
    }
}
