//! Candidatos a erro no relatório da transcrição (heurísticas explícitas do plano).
//!
//! 1. Grafia de português europeu com equivalente pt-BR.
//! 2. Inicial maiúscula fora do início de frase (provável nome próprio).
//! 3. Baixa confiança: o crate `sherpa-onnx` 2.0.1 não expõe confiança por token
//!    (S1), então esta heurística fica sem fonte por enquanto.
//! 4. Palavra curta de inglês/espanhol no meio do português (o Parakeet v3 não
//!    fixa o idioma; achado do S1).
//!
//! São só candidatos: nada é alterado no texto.

use crate::words::Word;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Grafia de Portugal.
    PtPt,
    /// Maiúscula no meio da frase.
    Name,
    /// Inglês ou espanhol no meio do português.
    Language,
}

#[derive(Clone, Debug, Serialize)]
pub struct Suspect {
    /// Índice da palavra na transcrição.
    pub idx: usize,
    pub start: f64,
    pub word: String,
    pub kind: Kind,
    pub hint: Option<String>,
}

/// pt-PT → pt-BR (formas mais comuns; a comparação ignora maiúsculas e pontuação).
const PT_PT: &[(&str, &str)] = &[
    ("equipa", "equipe"),
    ("equipas", "equipes"),
    ("facto", "fato"),
    ("factos", "fatos"),
    ("registo", "registro"),
    ("registos", "registros"),
    ("utilizador", "usuário"),
    ("utilizadores", "usuários"),
    ("utilizar", "usar"),
    ("ecrã", "tela"),
    ("ecrãs", "telas"),
    ("telemóvel", "celular"),
    ("telemóveis", "celulares"),
    ("autocarro", "ônibus"),
    ("comboio", "trem"),
    ("pequeno-almoço", "café da manhã"),
    ("casa de banho", "banheiro"),
    ("frigorífico", "geladeira"),
    ("rapariga", "moça"),
    ("miúdo", "garoto"),
    ("miúdos", "garotos"),
    ("sítio", "lugar/site"),
    ("ficheiro", "arquivo"),
    ("ficheiros", "arquivos"),
    ("rato", "mouse"),
    ("actual", "atual"),
    ("actualmente", "atualmente"),
    ("acção", "ação"),
    ("acções", "ações"),
    ("direcção", "direção"),
    ("objectivo", "objetivo"),
    ("objectivos", "objetivos"),
    ("projecto", "projeto"),
    ("projectos", "projetos"),
    ("óptimo", "ótimo"),
    ("percebes", "entende"),
    ("estás", "está"),
    ("tens", "tem"),
    ("fixe", "legal"),
    ("gajo", "cara"),
    ("bué", "muito"),
    ("connosco", "conosco"),
    ("contacto", "contato"),
    ("contactos", "contatos"),
    ("económico", "econômico"),
    ("género", "gênero"),
    ("prémio", "prêmio"),
    ("académico", "acadêmico"),
];

/// Palavras curtas de en/es que não existem (ou quase nunca aparecem) em pt-BR.
const OTHER_LANG: &[&str] = &[
    "but", "and", "or", "the", "of", "for", "with", "to", "is", "are", "was", "this", "that", "you", "we", "they", "have", "has", "not",
    "what", "which", "reunion", "pero", "muy", "también", "entonces", "ahora", "gracias", "hola", "cuando", "usted",
    "nosotros", "ellos", "hay", "eso", "esto", "bueno", "pues", "y", "el", "los", "las", "del", "una", "con", "sin", "hacer", "tiene",
];

fn norm(w: &str) -> String {
    w.trim_matches(|c: char| !c.is_alphanumeric() && c != '-').to_lowercase()
}

fn ends_sentence(w: &str) -> bool {
    w.trim_end_matches(['"', '\'', ')', '»', '”']).ends_with(['.', '?', '!', '…'])
}

pub fn find(words: &[Word]) -> Vec<Suspect> {
    let mut out = Vec::new();
    for (i, w) in words.iter().enumerate() {
        let n = norm(&w.word);
        if n.is_empty() {
            continue;
        }
        let push = |out: &mut Vec<Suspect>, kind, hint: Option<String>| out.push(Suspect { idx: i, start: w.start, word: w.word.clone(), kind, hint });
        if let Some((_, br)) = PT_PT.iter().find(|(pt, _)| *pt == n) {
            push(&mut out, Kind::PtPt, Some((*br).to_string()));
        } else if OTHER_LANG.contains(&n.as_str()) {
            push(&mut out, Kind::Language, None);
        } else {
            let sentence_start = i == 0 || ends_sentence(&words[i - 1].word);
            let first = w.word.trim_start_matches(|c: char| !c.is_alphanumeric()).chars().next();
            if !sentence_start && first.is_some_and(char::is_uppercase) {
                push(&mut out, Kind::Name, None);
            }
        }
    }
    out
}

/// Acima disso o relatório sugere a Parte 4 (comparar com o Whisper).
pub const MAX_PER_MIN: f64 = 3.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn ws(s: &str) -> Vec<Word> {
        s.split(' ').enumerate().map(|(i, w)| Word { word: w.into(), start: i as f64, end: i as f64 + 0.5 }).collect()
    }

    #[test]
    fn acha_os_tres_tipos() {
        let s = find(&ws("A equipa falou com o Marcos, but depois. Ontem foi bom"));
        let kinds: Vec<(&str, Kind)> = s.iter().map(|s| (s.word.as_str(), s.kind)).collect();
        assert_eq!(kinds, [("equipa", Kind::PtPt), ("Marcos,", Kind::Name), ("but", Kind::Language)]);
        assert_eq!(s[0].hint.as_deref(), Some("equipe"));
        assert_eq!(s[1].start, 5.0);
    }

    #[test]
    fn maiuscula_no_inicio_da_frase_nao_conta() {
        assert!(find(&ws("Bom dia. Hoje vamos")).is_empty());
    }
}
