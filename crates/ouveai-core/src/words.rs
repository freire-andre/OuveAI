//! Montagem das palavras a partir dos tokens do modelo (regra 2 do plano).
//!
//! - Token com `▁` ou espaço inicial abre palavra nova; token sem marcador
//!   (inclusive pontuação) gruda na anterior; o 1º token sempre abre palavra.
//! - Início = timestamp do 1º token.
//! - Fim, com `durations`: início + duração do último token **que não seja só
//!   pontuação** (no Parakeet a pontuação tem timestamp próprio, depois da pausa).
//! - Fim, sem `durations`: início da palavra seguinte, recuado ao começo do
//!   silêncio medido entre as duas, limitado a início + 1,5 s.
//! - A última palavra fica limitada à duração do áudio.
//!
//! Nenhum tempo é estimado: tudo vem do modelo ou do silêncio medido.

use serde::{Deserialize, Serialize};

/// Sem `durations`, nenhuma palavra passa de início + 1,5 s.
pub const MAX_SEM_DURACAO: f64 = 1.5;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub word: String,
    pub start: f64,
    pub end: f64,
}

/// Intervalo de silêncio medido (`silencedetect` em arquivo, VAD ao vivo).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pause {
    pub start: f64,
    pub end: f64,
}

/// Saída crua do reconhecedor para um trecho de áudio.
#[derive(Clone, Debug, Default)]
pub struct Tokens {
    pub tokens: Vec<String>,
    pub timestamps: Vec<f32>,
    pub durations: Option<Vec<f32>>,
}

fn is_marker(c: char) -> bool {
    c == '▁' || c == ' '
}

/// Só pontuação/símbolos (sem letra nem dígito).
pub fn is_punct(s: &str) -> bool {
    !s.trim_matches(is_marker).is_empty() && !s.chars().any(char::is_alphanumeric)
}

struct Building {
    text: String,
    start: f64,
    /// Fim do último token que não é só pontuação (com `durations`).
    end_tok: Option<f64>,
    /// Fim do último token, qualquer que seja (se a palavra for só pontuação).
    end_any: f64,
}

/// Monta as palavras. `offset` é somado a todos os tempos (início do pedaço no
/// arquivo); `audio_end` é o fim do áudio no mesmo eixo; `pauses` também.
pub fn assemble(t: &Tokens, offset: f64, audio_end: f64, pauses: &[Pause]) -> Vec<Word> {
    let n = t.tokens.len().min(t.timestamps.len());
    let durs = t.durations.as_ref().filter(|d| d.len() >= n);
    let mut out: Vec<Building> = Vec::new();
    let mut open_next = true; // o 1º token sempre abre palavra

    for i in 0..n {
        let raw = &t.tokens[i];
        let opens = open_next || raw.starts_with(is_marker);
        let piece = raw.trim_start_matches(is_marker);
        if piece.is_empty() {
            // Token só com o marcador: a próxima peça abre palavra.
            open_next = true;
            continue;
        }
        open_next = false;
        let start = offset + t.timestamps[i] as f64;
        let end = durs.map(|d| start + d[i] as f64);
        let punct = is_punct(piece);
        if opens || out.is_empty() {
            out.push(Building { text: piece.to_string(), start, end_tok: end.filter(|_| !punct), end_any: end.unwrap_or(start) });
        } else {
            let b = out.last_mut().unwrap();
            b.text.push_str(piece);
            if let Some(e) = end {
                b.end_any = e;
                if !punct {
                    b.end_tok = Some(e);
                }
            }
        }
    }

    let starts: Vec<f64> = out.iter().map(|b| b.start).collect();
    out.into_iter()
        .enumerate()
        .map(|(i, b)| {
            let next = starts.get(i + 1).copied();
            let mut end = if durs.is_some() {
                b.end_tok.unwrap_or(b.end_any)
            } else {
                let limit = b.start + MAX_SEM_DURACAO;
                match next {
                    Some(nx) => {
                        // Recua ao começo do 1º silêncio medido entre as duas palavras.
                        let silence = pauses.iter().filter(|p| p.start > b.start && p.start < nx).map(|p| p.start).fold(nx, f64::min);
                        silence.min(limit)
                    }
                    None => limit,
                }
            };
            if next.is_none() {
                end = end.min(audio_end);
            }
            Word { word: b.text, start: b.start, end: end.max(b.start) }
        })
        .collect()
}

/// Arredonda para milissegundos (o `.json` usa 3 casas).
pub fn ms(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(v: &[(&str, f32, f32)], with_dur: bool) -> Tokens {
        Tokens {
            tokens: v.iter().map(|t| t.0.to_string()).collect(),
            timestamps: v.iter().map(|t| t.1).collect(),
            durations: with_dur.then(|| v.iter().map(|t| t.2).collect()),
        }
    }

    #[test]
    fn primeiro_token_abre_palavra_mesmo_sem_marcador() {
        let w = assemble(&toks(&[("Ol", 0.0, 0.08), ("á", 0.08, 0.08), (" mundo", 0.4, 0.16)], true), 0.0, 10.0, &[]);
        assert_eq!(w.iter().map(|w| w.word.as_str()).collect::<Vec<_>>(), ["Olá", "mundo"]);
        assert_eq!(w[0].start, 0.0);
        assert!((w[0].end - 0.16).abs() < 1e-6);
    }

    #[test]
    fn marcador_sentencepiece_tambem_abre() {
        let w = assemble(&toks(&[("▁bom", 0.0, 0.08), ("▁dia", 0.24, 0.08)], true), 0.0, 10.0, &[]);
        assert_eq!(w.len(), 2);
        assert_eq!(w[1].word, "dia");
    }

    #[test]
    fn pontuacao_gruda_mas_nao_estica_o_fim() {
        // Achado do S1: a vírgula vem depois da pausa, com timestamp próprio.
        let w = assemble(&toks(&[(" reun", 2.0, 0.08), ("ião", 2.32, 0.08), (",", 4.16, 0.08), (" depois", 4.4, 0.16)], true), 0.0, 10.0, &[]);
        assert_eq!(w[0].word, "reunião,");
        assert!((w[0].end - 2.40).abs() < 1e-6, "fim {}", w[0].end);
    }

    #[test]
    fn sem_durations_recua_ao_silencio_e_limita() {
        let t = toks(&[(" a", 0.0, 0.0), (" b", 3.0, 0.0), (" c", 3.4, 0.0)], false);
        let pauses = [Pause { start: 0.6, end: 2.9 }];
        let w = assemble(&t, 0.0, 3.8, &pauses);
        assert!((w[0].end - 0.6).abs() < 1e-9); // recuou ao silêncio
        assert!((w[1].end - 3.4).abs() < 1e-5); // início da próxima (f32)
        assert!((w[2].end - 3.8).abs() < 1e-9); // limitado ao fim do áudio
        let w = assemble(&toks(&[(" a", 0.0, 0.0), (" b", 5.0, 0.0)], false), 0.0, 9.0, &[]);
        assert!((w[0].end - MAX_SEM_DURACAO).abs() < 1e-9);
    }

    #[test]
    fn offset_e_soma_e_ultima_palavra_limitada() {
        let w = assemble(&toks(&[(" fim", 9.92, 0.16)], true), 600.0, 610.0, &[]);
        assert!((w[0].start - 609.92).abs() < 1e-4);
        assert!((w[0].end - 610.0).abs() < 1e-9);
    }

    #[test]
    fn token_so_com_marcador_abre_a_proxima() {
        let w = assemble(&toks(&[(" um", 0.0, 0.08), (" ", 0.1, 0.0), ("dois", 0.3, 0.08)], true), 0.0, 5.0, &[]);
        assert_eq!(w.iter().map(|w| w.word.as_str()).collect::<Vec<_>>(), ["um", "dois"]);
    }
}
