//! Transcrição da reunião para fora do app: copiar (Markdown + HTML) e exportar
//! `.md`, `.txt`, `.srt` e `.json` (Fase 5).
//!
//! Regras (plano, Fase 5):
//! - falas seguidas do mesmo canal viram um bloco só, com o tempo da primeira;
//! - os marcadores ⭐ entram (troca de dispositivo e pausa, não);
//! - os ecos suspeitos ficam de fora (no `.json` eles vão, marcados);
//! - o tempo é relativo ao início, ou a hora do relógio se pedido.

use crate::store::{FLAG_ECHO, MarkRow, SegmentRow};
use crate::subs;
use crate::words::Word;
use serde::Serialize;

pub struct Meeting<'a> {
    pub title: &'a str,
    /// ISO em UTC, como no banco (`2026-10-31T14:02:00Z`).
    pub started_at: &'a str,
    pub duration_s: Option<f64>,
    pub segments: &'a [SegmentRow],
    pub marks: &'a [MarkRow],
    /// Palavras por segmento (`segment_id`, palavra), para a legenda e o `.json`.
    pub words: &'a [(i64, Word)],
}

/// `clock`: deslocamento do fuso em minutos a leste de UTC (Brasília = -180).
/// `None` = tempo relativo ao início da reunião.
#[derive(Clone, Copy, Default)]
pub struct Opts {
    pub clock: Option<i64>,
}

enum Block<'a> {
    Speech { t: f64, ch: &'a str, text: String },
    Star { t: f64, note: Option<&'a str> },
}

fn blocks<'a>(m: &'a Meeting) -> Vec<Block<'a>> {
    enum Item<'a> {
        Seg(&'a SegmentRow),
        Star(&'a MarkRow),
    }
    let mut items: Vec<(f64, Item)> = m.segments.iter().filter(|s| s.flags & FLAG_ECHO == 0).map(|s| (s.start_s, Item::Seg(s))).collect();
    items.extend(m.marks.iter().filter(|k| k.kind == "estrela").map(|k| (k.t_s, Item::Star(k))));
    items.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut out: Vec<Block> = Vec::new();
    for (t, it) in items {
        match it {
            Item::Star(k) => out.push(Block::Star { t, note: k.note.as_deref().filter(|n| !n.trim().is_empty()) }),
            Item::Seg(s) => {
                let text = s.text.trim();
                if text.is_empty() {
                    continue;
                }
                match out.last_mut() {
                    Some(Block::Speech { ch, text: acc, .. }) if *ch == s.channel => {
                        acc.push(' ');
                        acc.push_str(text);
                    }
                    _ => out.push(Block::Speech { t, ch: &s.channel, text: text.to_string() }),
                }
            }
        }
    }
    out
}

// ---------- datas sem dependência (algoritmos de Howard Hinnant) ----------

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub(crate) fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { yoe + era * 400 + 1 } else { yoe + era * 400 }, m, d)
}

/// `2026-10-31T14:02:05Z` → segundos Unix.
fn parse_iso(s: &str) -> Option<i64> {
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    Some(days_from_civil(n(0..4)?, n(5..7)?, n(8..10)?) * 86_400 + n(11..13)? * 3600 + n(14..16)? * 60 + n(17..19)?)
}

/// Data e hora locais "31/10/2026 11:02".
fn local_date(iso: &str, offset_min: i64) -> Option<String> {
    let t = parse_iso(iso)? + offset_min * 60;
    let (y, m, d) = civil_from_days(t.div_euclid(86_400));
    let s = t.rem_euclid(86_400);
    Some(format!("{d:02}/{m:02}/{y} {:02}:{:02}", s / 3600, s / 60 % 60))
}

fn hms(s: i64) -> String {
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

fn stamp(m: &Meeting, o: Opts, t: f64) -> String {
    match o.clock.and_then(|off| Some(parse_iso(m.started_at)? + off * 60)) {
        Some(base) => hms((base + t.max(0.0) as i64).rem_euclid(86_400)),
        None => hms(t.max(0.0) as i64),
    }
}

fn subtitle(m: &Meeting, o: Opts) -> String {
    let mut parts = Vec::new();
    // Sem fuso informado, a data sai em UTC (só a hora relativa importa no corpo).
    if let Some(d) = local_date(m.started_at, o.clock.unwrap_or(0)) {
        parts.push(d);
    }
    if let Some(d) = m.duration_s.filter(|d| *d > 0.0) {
        let min = (d / 60.0).round() as i64;
        parts.push(if min < 1 { format!("{} s", d.round() as i64) } else { format!("{min} min") });
    }
    parts.join(" · ")
}

fn star_label(note: Option<&str>) -> String {
    match note {
        Some(n) => format!("⭐ Marcado: {n}"),
        None => "⭐ Marcado".into(),
    }
}

/// Markdown no formato `**[00:12:03] OUTROS:** texto`. `header` põe o título e a
/// data (arquivo `.md`); ao copiar, vão só as falas.
pub fn markdown(m: &Meeting, o: Opts, header: bool) -> String {
    let mut out = Vec::new();
    if header {
        out.push(format!("# {}", m.title));
        out.push(format!("_{}_", subtitle(m, o)));
    }
    for b in blocks(m) {
        out.push(match b {
            Block::Speech { t, ch, text } => format!("**[{}] {ch}:** {text}", stamp(m, o, t)),
            Block::Star { t, note } => format!("**[{}] {}**", stamp(m, o, t), star_label(note)),
        });
    }
    let mut s = out.join("\n\n");
    s.push('\n');
    s
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// O mesmo conteúdo do Markdown em HTML: é o que o Teams e o Outlook usam ao colar,
/// e o negrito aparece formatado.
pub fn html(m: &Meeting, o: Opts) -> String {
    let mut s = String::new();
    for b in blocks(m) {
        match b {
            Block::Speech { t, ch, text } => s.push_str(&format!("<p><b>[{}] {ch}:</b> {}</p>", stamp(m, o, t), esc(&text))),
            Block::Star { t, note } => s.push_str(&format!("<p><b>[{}] {}</b></p>", stamp(m, o, t), esc(&star_label(note)))),
        }
    }
    s
}

pub fn txt(m: &Meeting, o: Opts) -> String {
    let mut out = vec![m.title.to_string(), subtitle(m, o), String::new()];
    for b in blocks(m) {
        out.push(match b {
            Block::Speech { t, ch, text } => format!("[{}] {ch}: {text}", stamp(m, o, t)),
            Block::Star { t, note } => format!("[{}] {}", stamp(m, o, t), star_label(note)),
        });
    }
    let mut s = out.join("\r\n");
    s.push_str("\r\n");
    s
}

/// Legenda com o canal na frente ("OUTROS: …"). Cada fala é quebrada pelas
/// mesmas regras da legenda de arquivo; falas de canais diferentes podem se sobrepor.
pub fn srt(m: &Meeting) -> String {
    let mut cues = Vec::new();
    for s in m.segments.iter().filter(|s| s.flags & FLAG_ECHO == 0) {
        let words: Vec<Word> = m.words.iter().filter(|(id, _)| *id == s.id).map(|(_, w)| w.clone()).collect();
        let parts = if words.is_empty() { vec![subs::Cue { start: s.start_s, end: s.end_s, text: s.text.clone() }] } else { subs::segment(&words, &[]) };
        cues.extend(parts.into_iter().map(|c| subs::Cue { text: format!("{}: {}", s.channel, c.text), ..c }));
    }
    cues.sort_by(|a, b| a.start.total_cmp(&b.start));
    subs::to_srt(&cues)
}

#[derive(Serialize)]
struct JsonWord<'a> {
    word: &'a str,
    start: f64,
    end: f64,
}

#[derive(Serialize)]
struct JsonSeg<'a> {
    channel: &'a str,
    start: f64,
    end: f64,
    text: &'a str,
    echo: bool,
    words: Vec<JsonWord<'a>>,
}

#[derive(Serialize)]
struct JsonMark<'a> {
    t: f64,
    kind: &'a str,
    note: Option<&'a str>,
}

#[derive(Serialize)]
struct JsonDoc<'a> {
    title: &'a str,
    started_at: &'a str,
    duration_s: Option<f64>,
    segments: Vec<JsonSeg<'a>>,
    marks: Vec<JsonMark<'a>>,
}

/// Tudo, inclusive os ecos (com `echo: true`) e as palavras com tempo.
pub fn json(m: &Meeting) -> String {
    let doc = JsonDoc {
        title: m.title,
        started_at: m.started_at,
        duration_s: m.duration_s,
        segments: m
            .segments
            .iter()
            .map(|s| JsonSeg {
                channel: &s.channel,
                start: s.start_s,
                end: s.end_s,
                text: &s.text,
                echo: s.flags & FLAG_ECHO != 0,
                words: m.words.iter().filter(|(id, _)| *id == s.id).map(|(_, w)| JsonWord { word: &w.word, start: w.start, end: w.end }).collect(),
            })
            .collect(),
        marks: m.marks.iter().map(|k| JsonMark { t: k.t_s, kind: &k.kind, note: k.note.as_deref() }).collect(),
    };
    serde_json::to_string_pretty(&doc).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(id: i64, ch: &str, start: f64, text: &str, flags: i64) -> SegmentRow {
        SegmentRow { id, channel: ch.into(), start_s: start, end_s: start + 2.0, text: text.into(), flags }
    }

    fn fixture() -> (Vec<SegmentRow>, Vec<MarkRow>, Vec<(i64, Word)>) {
        let segs = vec![
            seg(1, "OUTROS", 3.0, "Bom dia.", 0),
            seg(2, "OUTROS", 6.0, "Vamos começar?", 0),
            seg(3, "EU", 9.5, "Bom dia <todos>", 0),
            seg(4, "EU", 11.0, "eco do outro", FLAG_ECHO),
            seg(5, "OUTROS", 723.0, "Fechado.", 0),
        ];
        let marks = vec![
            MarkRow { id: 1, t_s: 10.0, kind: "estrela".into(), note: None },
            MarkRow { id: 2, t_s: 12.0, kind: "pausa".into(), note: None },
            MarkRow { id: 3, t_s: 700.0, kind: "dispositivo".into(), note: Some("Fone".into()) },
        ];
        let w = |word: &str, start: f64, end: f64| Word { word: word.into(), start, end };
        let words = vec![(1, w("Bom", 3.0, 3.3)), (1, w("dia.", 3.3, 3.7)), (5, w("Fechado.", 723.0, 723.6))];
        (segs, marks, words)
    }

    fn meeting<'a>(f: &'a (Vec<SegmentRow>, Vec<MarkRow>, Vec<(i64, Word)>)) -> Meeting<'a> {
        Meeting { title: "Reunião", started_at: "2026-10-31T23:59:30Z", duration_s: Some(730.0), segments: &f.0, marks: &f.1, words: &f.2 }
    }

    #[test]
    fn markdown_agrupa_inclui_estrela_e_tira_eco() {
        let f = fixture();
        let md = markdown(&meeting(&f), Opts::default(), false);
        assert_eq!(
            md,
            "**[00:00:03] OUTROS:** Bom dia. Vamos começar?\n\n\
             **[00:00:09] EU:** Bom dia <todos>\n\n\
             **[00:00:10] ⭐ Marcado**\n\n\
             **[00:12:03] OUTROS:** Fechado.\n"
        );
        let with_header = markdown(&meeting(&f), Opts { clock: Some(-180) }, true);
        assert!(with_header.starts_with("# Reunião\n\n_31/10/2026 20:59 · 12 min_\n\n"), "{with_header}");
    }

    #[test]
    fn hora_do_relogio_vira_o_dia() {
        let f = fixture();
        // Começou às 23:59:30 UTC: a fala aos 723 s já é no dia seguinte.
        let md = markdown(&meeting(&f), Opts { clock: Some(0) }, false);
        assert!(md.starts_with("**[23:59:33] OUTROS:**"), "{md}");
        assert!(md.contains("**[00:11:33] OUTROS:** Fechado."), "{md}");
        let md = markdown(&meeting(&f), Opts { clock: Some(-180) }, false);
        assert!(md.starts_with("**[20:59:33] OUTROS:**"), "{md}");
        assert_eq!(local_date("2026-01-01T01:00:00Z", -180).unwrap(), "31/12/2025 22:00");
        assert_eq!(parse_iso("1970-01-02T00:00:01Z"), Some(86_401));
    }

    #[test]
    fn html_escapa_e_txt_srt_json() {
        let f = fixture();
        let m = meeting(&f);
        let h = html(&m, Opts::default());
        assert!(h.contains("<p><b>[00:00:09] EU:</b> Bom dia &lt;todos&gt;</p>"), "{h}");
        assert!(!h.contains("eco do outro"));
        let t = txt(&m, Opts::default());
        assert!(t.contains("\r\n[00:00:03] OUTROS: Bom dia. Vamos começar?\r\n"), "{t}");
        let s = srt(&m);
        assert!(s.starts_with("1\r\n00:00:03,000 --> "), "{s}");
        assert!(s.contains("OUTROS: Bom dia.\r\n"), "{s}");
        assert!(s.contains("EU: Bom dia <todos>"), "segmento sem palavras usa o texto inteiro");
        assert!(!s.contains("eco do outro"));
        let j: serde_json::Value = serde_json::from_str(&json(&m)).unwrap();
        assert_eq!(j["segments"].as_array().unwrap().len(), 5);
        assert_eq!(j["segments"][3]["echo"], true);
        assert_eq!(j["segments"][0]["words"][1]["word"], "dia.");
        assert_eq!(j["marks"][2]["note"], "Fone");
    }
}
