//! Banco local (SQLite, WAL + `synchronous=NORMAL`): configurações e sessões.
//!
//! Com WAL, matar o processo não perde transações confirmadas. No início, as
//! sessões que ficaram `recording`/`paused` viram `interrupted` (texto preservado).

use crate::transcribe::Transcript;
use crate::words::{Pause, Word};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::path::Path;

/// Migrações em ordem; o índice + 1 é o `user_version` depois de aplicá-la.
const MIGRATIONS: &[&str] = &[
    "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;",
    // Fase 3: modelo de dados da seção 9 do plano. `pauses` guarda os silêncios
    // medidos do arquivo (JSON), para regenerar a legenda depois de corrigir o texto.
    "CREATE TABLE sessions (
        id INTEGER PRIMARY KEY,
        title TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (kind IN ('meeting','file')),
        source_path TEXT,
        started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
        ended_at TEXT,
        status TEXT NOT NULL CHECK (status IN ('recording','paused','ended','interrupted')),
        duration_s REAL,
        audio_dir TEXT,
        audio_expires_at TEXT,
        app_version TEXT,
        pauses TEXT
    ) STRICT;
    CREATE TABLE segments (
        id INTEGER PRIMARY KEY,
        session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        channel TEXT NOT NULL CHECK (channel IN ('EU','OUTROS','ARQUIVO')),
        speaker_label TEXT,
        start_s REAL NOT NULL,
        end_s REAL NOT NULL,
        text TEXT NOT NULL,
        flags INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE INDEX segments_session ON segments(session_id, start_s);
    CREATE TABLE words (
        id INTEGER PRIMARY KEY,
        segment_id INTEGER NOT NULL REFERENCES segments(id) ON DELETE CASCADE,
        idx INTEGER NOT NULL,
        word TEXT NOT NULL,
        start_s REAL NOT NULL,
        end_s REAL NOT NULL
    ) STRICT;
    CREATE INDEX words_segment ON words(segment_id, idx);",
    // Fase 4: marcadores da reunião (estrela, troca de dispositivo, pausa).
    "CREATE TABLE marks (
        id INTEGER PRIMARY KEY,
        session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        t_s REAL NOT NULL,
        kind TEXT NOT NULL CHECK (kind IN ('estrela','dispositivo','pausa')),
        note TEXT
    ) STRICT;
    CREATE INDEX marks_session ON marks(session_id, t_s);",
    // Fase 5: busca sem diferenciar acento, sincronizada por triggers (inclusive no
    // ON DELETE CASCADE da sessão).
    "CREATE VIRTUAL TABLE segments_fts USING fts5(
        text, content='segments', content_rowid='id', tokenize='unicode61 remove_diacritics 2'
    );
    CREATE TRIGGER segments_ai AFTER INSERT ON segments BEGIN
        INSERT INTO segments_fts(rowid, text) VALUES (new.id, new.text);
    END;
    CREATE TRIGGER segments_ad AFTER DELETE ON segments BEGIN
        INSERT INTO segments_fts(segments_fts, rowid, text) VALUES ('delete', old.id, old.text);
    END;
    CREATE TRIGGER segments_au AFTER UPDATE OF text ON segments BEGIN
        INSERT INTO segments_fts(segments_fts, rowid, text) VALUES ('delete', old.id, old.text);
        INSERT INTO segments_fts(rowid, text) VALUES (new.id, new.text);
    END;
    INSERT INTO segments_fts(segments_fts) VALUES ('rebuild');",
];

/// Marcadores do trecho encontrado em `Hit::snippet` (o frontend troca por `<mark>`).
pub const HIT_START: char = '\u{2}';
pub const HIT_END: char = '\u{3}';

/// `segments.flags`
pub const FLAG_ECHO: i64 = 1;
pub const FLAG_EDITED: i64 = 2;

#[derive(Clone, Debug, Serialize)]
pub struct SegmentRow {
    pub id: i64,
    pub channel: String,
    pub start_s: f64,
    pub end_s: f64,
    pub text: String,
    pub flags: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MarkRow {
    pub id: i64,
    pub t_s: f64,
    pub kind: String,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionRow {
    pub id: i64,
    pub title: String,
    pub kind: String,
    pub source_path: Option<String>,
    pub started_at: String,
    pub status: String,
    pub duration_s: Option<f64>,
    pub words: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Hit {
    pub session_id: i64,
    pub kind: String,
    pub title: String,
    pub started_at: String,
    pub segment_id: i64,
    pub channel: String,
    pub start_s: f64,
    /// Trecho com os termos entre `HIT_START` e `HIT_END`.
    pub snippet: String,
}

/// Consulta FTS5 a partir do que a pessoa digitou: cada termo vira prefixo
/// (`"reun"*`) e todos precisam aparecer. Pontuação e aspas são descartadas,
/// então a entrada nunca quebra a sintaxe do MATCH.
pub fn fts_query(q: &str) -> Option<String> {
    let terms: Vec<String> = q.split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()).map(|t| format!("\"{t}\"*")).collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

fn ends_sentence(w: &str) -> bool {
    w.trim_end_matches(['"', '\'', ')', '»', '”']).ends_with(['.', '?', '!', '…'])
}

/// Agrupa as palavras de um arquivo em segmentos: frase terminada, silêncio
/// medido de 1 s ou mais, ou 30 s de fala corrida.
fn group_segments(words: &[Word], pauses: &[Pause]) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut from = 0;
    for i in 1..=words.len() {
        let cut = i == words.len() || {
            let (prev, w) = (&words[i - 1], &words[i]);
            ends_sentence(&prev.word) || crate::subs::silence_between(pauses, prev.end, w.start) >= 1.0 || w.end - words[from].start > 30.0
        };
        if cut {
            out.push(from..i);
            from = i;
        }
    }
    out
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        let mut s = Self { conn };
        s.migrate()?;
        Ok(s)
    }

    fn migrate(&mut self) -> rusqlite::Result<()> {
        let v: i64 = self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(v as usize) {
            let tx = self.conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", i as i64 + 1)?;
            tx.commit()?;
        }
        Ok(())
    }

    pub fn get(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0))
            .optional()
    }

    pub fn set(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn all(&self) -> rusqlite::Result<Vec<(String, String)>> {
        let mut st = self.conn.prepare("SELECT key, value FROM settings ORDER BY key")?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()
    }

    /// Sessões que ficaram gravando/pausadas (o app caiu) viram `interrupted`.
    /// A duração vira o fim da última fala, e o áudio que ficou recebe a validade
    /// atual (`keep_days`; `None` = sem validade).
    pub fn recover_orphans(&self, keep_days: Option<i64>) -> rusqlite::Result<usize> {
        self.conn.execute(
            "UPDATE sessions SET status = 'interrupted', ended_at = COALESCE(ended_at, strftime('%Y-%m-%dT%H:%M:%SZ','now')),
                duration_s = COALESCE(duration_s, (SELECT MAX(end_s) FROM segments WHERE session_id = sessions.id)),
                audio_expires_at = CASE WHEN audio_dir IS NULL OR ?1 IS NULL THEN audio_expires_at
                    ELSE strftime('%Y-%m-%dT%H:%M:%SZ','now', '+' || ?1 || ' days') END
             WHERE status IN ('recording','paused')",
            params![keep_days],
        )
    }

    /// O arquivo importado vira uma sessão `kind='file'`, numa transação só.
    pub fn create_file_session(&mut self, title: &str, source_path: &Path, t: &Transcript, app_version: &str) -> rusqlite::Result<i64> {
        let pauses: Vec<[f64; 2]> = t.pauses.iter().map(|p| [p.start, p.end]).collect();
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO sessions (title, kind, source_path, ended_at, status, duration_s, app_version, pauses)
             VALUES (?1, 'file', ?2, strftime('%Y-%m-%dT%H:%M:%SZ','now'), 'ended', ?3, ?4, ?5)",
            params![title, source_path.to_string_lossy(), t.duration, app_version, serde_json::to_string(&pauses).unwrap()],
        )?;
        let sid = tx.last_insert_rowid();
        {
            let mut seg = tx.prepare("INSERT INTO segments (session_id, channel, start_s, end_s, text) VALUES (?1, 'ARQUIVO', ?2, ?3, ?4)")?;
            let mut wd = tx.prepare("INSERT INTO words (segment_id, idx, word, start_s, end_s) VALUES (?1, ?2, ?3, ?4, ?5)")?;
            for r in group_segments(&t.words, &t.pauses) {
                let ws = &t.words[r.clone()];
                let text = ws.iter().map(|w| w.word.as_str()).collect::<Vec<_>>().join(" ");
                seg.execute(params![sid, ws[0].start, ws[ws.len() - 1].end, text])?;
                let seg_id = tx.last_insert_rowid();
                for (k, w) in ws.iter().enumerate() {
                    wd.execute(params![seg_id, (r.start + k) as i64, w.word, w.start, w.end])?;
                }
            }
        }
        tx.commit()?;
        Ok(sid)
    }

    pub fn sessions(&self, kind: Option<&str>) -> rusqlite::Result<Vec<SessionRow>> {
        let mut st = self.conn.prepare(
            "SELECT s.id, s.title, s.kind, s.source_path, s.started_at, s.status, s.duration_s,
                    (SELECT COUNT(*) FROM words w JOIN segments g ON g.id = w.segment_id WHERE g.session_id = s.id)
             FROM sessions s WHERE ?1 IS NULL OR s.kind = ?1 ORDER BY s.started_at DESC, s.id DESC",
        )?;
        st.query_map(params![kind], |r| {
            Ok(SessionRow {
                id: r.get(0)?,
                title: r.get(1)?,
                kind: r.get(2)?,
                source_path: r.get(3)?,
                started_at: r.get(4)?,
                status: r.get(5)?,
                duration_s: r.get(6)?,
                words: r.get(7)?,
            })
        })?
        .collect()
    }

    pub fn session(&self, id: i64) -> rusqlite::Result<Option<SessionRow>> {
        Ok(self.sessions(None)?.into_iter().find(|s| s.id == id))
    }

    /// Remonta o `Transcript` (palavras na ordem + pausas medidas) de uma sessão.
    pub fn transcript(&self, id: i64) -> rusqlite::Result<Option<Transcript>> {
        let Some(row) = self.session(id)? else { return Ok(None) };
        let pauses: Option<String> = self.conn.query_row("SELECT pauses FROM sessions WHERE id = ?1", [id], |r| r.get(0))?;
        let pauses: Vec<[f64; 2]> = pauses.and_then(|p| serde_json::from_str(&p).ok()).unwrap_or_default();
        let mut st = self.conn.prepare(
            "SELECT w.word, w.start_s, w.end_s FROM words w JOIN segments g ON g.id = w.segment_id
             WHERE g.session_id = ?1 ORDER BY w.idx",
        )?;
        let words = st.query_map([id], |r| Ok(Word { word: r.get(0)?, start: r.get(1)?, end: r.get(2)? }))?.collect::<Result<Vec<_>, _>>()?;
        let source = row
            .source_path
            .as_deref()
            .map(|p| Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
            .unwrap_or_default();
        Ok(Some(Transcript {
            source,
            duration: row.duration_s.unwrap_or(0.0),
            words,
            pauses: pauses.into_iter().map(|[start, end]| Pause { start, end }).collect(),
        }))
    }

    /// Corrige só o texto de uma palavra (nunca os tempos) e refaz o texto do segmento.
    pub fn update_word(&mut self, session_id: i64, idx: i64, text: &str) -> rusqlite::Result<bool> {
        let text = text.trim();
        if text.is_empty() || text.contains(char::is_whitespace) {
            return Err(rusqlite::Error::InvalidParameterName("a palavra não pode ficar vazia nem ter espaço".into()));
        }
        let tx = self.conn.transaction()?;
        let seg: Option<i64> = tx
            .query_row(
                "SELECT w.segment_id FROM words w JOIN segments g ON g.id = w.segment_id WHERE g.session_id = ?1 AND w.idx = ?2",
                params![session_id, idx],
                |r| r.get(0),
            )
            .optional()?;
        let Some(seg) = seg else { return Ok(false) };
        tx.execute("UPDATE words SET word = ?1 WHERE segment_id = ?2 AND idx = ?3", params![text, seg, idx])?;
        tx.execute(
            "UPDATE segments SET flags = flags | ?2,
                text = (SELECT group_concat(word, ' ') FROM (SELECT word FROM words WHERE segment_id = ?1 ORDER BY idx))
             WHERE id = ?1",
            params![seg, FLAG_EDITED],
        )?;
        tx.commit()?;
        Ok(true)
    }

    // ---------- reunião (Fase 4) ----------

    pub fn create_meeting(&self, title: &str, app_version: &str) -> rusqlite::Result<i64> {
        self.conn.execute("INSERT INTO sessions (title, kind, status, app_version) VALUES (?1, 'meeting', 'recording', ?2)", params![title, app_version])?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Grava um segmento com as palavras, numa transação (`idx` contínuo na sessão).
    pub fn add_segment(&mut self, session_id: i64, channel: &str, text: &str, flags: i64, words: &[Word]) -> rusqlite::Result<i64> {
        let (Some(first), Some(last)) = (words.first(), words.last()) else {
            return Err(rusqlite::Error::InvalidParameterName("segmento sem palavras".into()));
        };
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO segments (session_id, channel, start_s, end_s, text, flags) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![session_id, channel, first.start, last.end, text, flags],
        )?;
        let seg = tx.last_insert_rowid();
        let next: i64 = tx.query_row(
            "SELECT COALESCE(MAX(w.idx) + 1, 0) FROM words w JOIN segments g ON g.id = w.segment_id WHERE g.session_id = ?1",
            [session_id],
            |r| r.get(0),
        )?;
        {
            let mut st = tx.prepare("INSERT INTO words (segment_id, idx, word, start_s, end_s) VALUES (?1, ?2, ?3, ?4, ?5)")?;
            for (k, w) in words.iter().enumerate() {
                st.execute(params![seg, next + k as i64, w.word, w.start, w.end])?;
            }
        }
        tx.commit()?;
        Ok(seg)
    }

    pub fn set_segment_flag(&self, segment_id: i64, flag: i64, on: bool) -> rusqlite::Result<()> {
        let sql = if on { "UPDATE segments SET flags = flags | ?2 WHERE id = ?1" } else { "UPDATE segments SET flags = flags & ~?2 WHERE id = ?1" };
        self.conn.execute(sql, params![segment_id, flag])?;
        Ok(())
    }

    pub fn add_mark(&self, session_id: i64, t_s: f64, kind: &str, note: Option<&str>) -> rusqlite::Result<i64> {
        self.conn.execute("INSERT INTO marks (session_id, t_s, kind, note) VALUES (?1, ?2, ?3, ?4)", params![session_id, t_s, kind, note])?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Pasta do áudio, gravada já no início: se o app cair, a sessão interrompida
    /// continua apontando para o áudio (e ele vence como os outros).
    pub fn set_audio_dir(&self, session_id: i64, dir: &str) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE sessions SET audio_dir = ?2 WHERE id = ?1", params![session_id, dir])?;
        Ok(())
    }

    pub fn set_status(&self, session_id: i64, status: &str) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE sessions SET status = ?2 WHERE id = ?1", params![session_id, status])?;
        Ok(())
    }

    /// `keep_days`: validade do áudio salvo (D2); `None` = sem validade ("sempre") ou sem áudio.
    pub fn end_meeting(&self, session_id: i64, duration_s: f64, audio_dir: Option<&str>, keep_days: Option<i64>) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE sessions SET status = 'ended', ended_at = strftime('%Y-%m-%dT%H:%M:%SZ','now'), duration_s = ?2, audio_dir = ?3,
                audio_expires_at = CASE WHEN ?4 IS NULL THEN NULL ELSE strftime('%Y-%m-%dT%H:%M:%SZ','now', '+' || ?4 || ' days') END
             WHERE id = ?1",
            params![session_id, duration_s, audio_dir, keep_days],
        )?;
        Ok(())
    }

    pub fn segments(&self, session_id: i64) -> rusqlite::Result<Vec<SegmentRow>> {
        let mut st = self.conn.prepare("SELECT id, channel, start_s, end_s, text, flags FROM segments WHERE session_id = ?1 ORDER BY start_s, id")?;
        st.query_map([session_id], |r| Ok(SegmentRow { id: r.get(0)?, channel: r.get(1)?, start_s: r.get(2)?, end_s: r.get(3)?, text: r.get(4)?, flags: r.get(5)? }))?
            .collect()
    }

    pub fn marks(&self, session_id: i64) -> rusqlite::Result<Vec<MarkRow>> {
        let mut st = self.conn.prepare("SELECT id, t_s, kind, note FROM marks WHERE session_id = ?1 ORDER BY t_s")?;
        st.query_map([session_id], |r| Ok(MarkRow { id: r.get(0)?, t_s: r.get(1)?, kind: r.get(2)?, note: r.get(3)? }))?.collect()
    }

    /// Pasta do áudio salvo da reunião, e até quando vale (`None` = sempre).
    pub fn audio(&self, session_id: i64) -> rusqlite::Result<Option<(String, Option<String>)>> {
        let r: (Option<String>, Option<String>) =
            self.conn.query_row("SELECT audio_dir, audio_expires_at FROM sessions WHERE id = ?1", [session_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(r.0.map(|d| (d, r.1)))
    }

    /// Áudios vencidos (D2): tira do banco e devolve as pastas para apagar.
    pub fn take_expired_audio(&self) -> rusqlite::Result<Vec<String>> {
        const DUE: &str = "audio_dir IS NOT NULL AND audio_expires_at IS NOT NULL AND audio_expires_at <= strftime('%Y-%m-%dT%H:%M:%SZ','now')";
        let dirs = self.conn.prepare(&format!("SELECT audio_dir FROM sessions WHERE {DUE}"))?.query_map([], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
        self.conn.execute(&format!("UPDATE sessions SET audio_dir = NULL, audio_expires_at = NULL WHERE {DUE}"), [])?;
        Ok(dirs)
    }

    /// Palavras de cada segmento da sessão, na ordem (para a legenda da reunião).
    pub fn segment_words(&self, session_id: i64) -> rusqlite::Result<Vec<(i64, Word)>> {
        let mut st = self.conn.prepare(
            "SELECT w.segment_id, w.word, w.start_s, w.end_s FROM words w JOIN segments g ON g.id = w.segment_id
             WHERE g.session_id = ?1 ORDER BY w.idx",
        )?;
        st.query_map([session_id], |r| Ok((r.get(0)?, Word { word: r.get(1)?, start: r.get(2)?, end: r.get(3)? })))?.collect()
    }

    /// Busca nas falas de todas as sessões, sem diferenciar acento nem maiúscula.
    /// Ecos ficam de fora. Mais recentes primeiro.
    pub fn search(&self, q: &str, limit: usize) -> rusqlite::Result<Vec<Hit>> {
        let Some(m) = fts_query(q) else { return Ok(Vec::new()) };
        let mut st = self.conn.prepare(
            "SELECT s.id, s.kind, s.title, s.started_at, g.id, g.channel, g.start_s,
                    snippet(segments_fts, 0, char(2), char(3), '…', 14)
             FROM segments_fts JOIN segments g ON g.id = segments_fts.rowid JOIN sessions s ON s.id = g.session_id
             WHERE segments_fts MATCH ?1 AND (g.flags & ?2) = 0
             ORDER BY s.started_at DESC, s.id DESC, g.start_s LIMIT ?3",
        )?;
        st.query_map(params![m, FLAG_ECHO, limit as i64], |r| {
            Ok(Hit {
                session_id: r.get(0)?,
                kind: r.get(1)?,
                title: r.get(2)?,
                started_at: r.get(3)?,
                segment_id: r.get(4)?,
                channel: r.get(5)?,
                start_s: r.get(6)?,
                snippet: r.get(7)?,
            })
        })?
        .collect()
    }

    /// Contagem de sessões por tipo: (reuniões, arquivos).
    pub fn counts(&self) -> rusqlite::Result<(i64, i64)> {
        self.conn.query_row(
            "SELECT COALESCE(SUM(kind = 'meeting'), 0), COALESCE(SUM(kind = 'file'), 0) FROM sessions",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
    }

    /// "Excluir tudo": apaga todas as sessões (texto, marcas) e devolve as pastas
    /// de áudio para apagar. Configurações e modelos ficam.
    pub fn delete_all(&mut self) -> rusqlite::Result<Vec<String>> {
        let tx = self.conn.transaction()?;
        let dirs = tx.prepare("SELECT audio_dir FROM sessions WHERE audio_dir IS NOT NULL")?.query_map([], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
        tx.execute("DELETE FROM sessions", [])?;
        tx.commit()?;
        Ok(dirs)
    }

    pub fn rename_session(&self, id: i64, title: &str) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE sessions SET title = ?2 WHERE id = ?1", params![id, title])?;
        Ok(())
    }

    pub fn delete_session(&self, id: i64) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM sessions WHERE id = ?1", [id])?;
        Ok(())
    }

    #[cfg(test)]
    fn exec(&self, sql: &str) {
        self.conn.execute_batch(sql).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_persistem_e_migracao_e_idempotente() {
        let dir = std::env::temp_dir().join(format!("ouveai-store-{}", std::process::id()));
        let db = dir.join("t.db");
        {
            let s = Store::open(&db).unwrap();
            assert_eq!(s.get("stealth").unwrap(), None);
            s.set("stealth", "true").unwrap();
            s.set("stealth", "false").unwrap();
        }
        let s = Store::open(&db).unwrap();
        assert_eq!(s.get("stealth").unwrap().as_deref(), Some("false"));
        assert_eq!(s.all().unwrap().len(), 1);
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }

    fn tmp(name: &str) -> (std::path::PathBuf, Store) {
        let dir = std::env::temp_dir().join(format!("ouveai-store-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let s = Store::open(&dir.join("t.db")).unwrap();
        (dir, s)
    }

    fn transcript() -> Transcript {
        let w = |word: &str, start: f64, end: f64| Word { word: word.into(), start, end };
        Transcript {
            source: "a.mp4".into(),
            duration: 10.0,
            words: vec![w("Oi.", 0.0, 0.4), w("Tudo", 1.0, 1.3), w("bem", 1.3, 1.6), w("depois", 5.0, 5.4)],
            pauses: vec![Pause { start: 1.7, end: 4.9 }],
        }
    }

    #[test]
    fn sessao_de_arquivo_ida_e_volta_e_edicao() {
        let (dir, mut s) = tmp("file");
        let id = s.create_file_session("a", std::path::Path::new(r"C:\v\a.mp4"), &transcript(), "0.0.1").unwrap();
        let rows = s.sessions(Some("file")).unwrap();
        assert_eq!((rows.len(), rows[0].words, rows[0].status.as_str()), (1, 4, "ended"));
        let t = s.transcript(id).unwrap().unwrap();
        assert_eq!(t.words, transcript().words);
        assert_eq!(t.pauses, transcript().pauses);
        // 3 segmentos: "Oi." | "Tudo bem" (silêncio de 3,2 s) | "depois"
        let n: i64 = s.conn.query_row("SELECT COUNT(*) FROM segments WHERE session_id = ?1", [id], |r| r.get(0)).unwrap();
        assert_eq!(n, 3);
        assert!(s.update_word(id, 2, "bem?").unwrap());
        let (text, flags): (String, i64) = s
            .conn
            .query_row("SELECT text, flags FROM segments WHERE session_id = ?1 AND start_s = 1.0", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!((text.as_str(), flags & FLAG_EDITED), ("Tudo bem?", FLAG_EDITED));
        let t = s.transcript(id).unwrap().unwrap();
        assert_eq!((t.words[2].word.as_str(), t.words[2].start, t.words[2].end), ("bem?", 1.3, 1.6)); // tempos intactos
        assert!(s.update_word(id, 2, "dois termos").is_err());
        s.delete_session(id).unwrap();
        let n: i64 = s.conn.query_row("SELECT COUNT(*) FROM words", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0); // ON DELETE CASCADE
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reuniao_grava_por_segmento_com_marcas_e_eco() {
        let (dir, mut s) = tmp("meeting");
        let id = s.create_meeting("Reunião", "0.0.1").unwrap();
        let w = |word: &str, start: f64, end: f64| Word { word: word.into(), start, end };
        let a = s.add_segment(id, "OUTROS", "então fecha", 0, &[w("então", 1.0, 1.3), w("fecha", 1.3, 1.7)]).unwrap();
        let b = s.add_segment(id, "EU", "fechado", 0, &[w("fechado", 2.0, 2.5)]).unwrap();
        assert!(s.add_segment(id, "EU", "", 0, &[]).is_err());
        s.set_segment_flag(b, FLAG_ECHO, true).unwrap();
        s.add_mark(id, 2.2, "estrela", None).unwrap();
        s.add_mark(id, 1.1, "dispositivo", Some("Fone")).unwrap();
        let segs = s.segments(id).unwrap();
        assert_eq!((segs.len(), segs[0].id, segs[1].flags), (2, a, FLAG_ECHO));
        let marks = s.marks(id).unwrap();
        assert_eq!((marks[0].kind.as_str(), marks[1].kind.as_str()), ("dispositivo", "estrela"));
        // idx contínuo na sessão: a palavra 2 é "fechado".
        assert!(s.update_word(id, 2, "Fechado.").unwrap());
        assert_eq!(s.sessions(Some("meeting")).unwrap()[0].status, "recording");
        s.end_meeting(id, 3.0, Some("C:/r/1"), Some(7)).unwrap();
        let exp: Option<String> = s.conn.query_row("SELECT audio_expires_at FROM sessions WHERE id = ?1", [id], |r| r.get(0)).unwrap();
        assert!(exp.is_some_and(|e| e.len() == 20 && e.ends_with('Z')));
        let row = s.session(id).unwrap().unwrap();
        assert_eq!((row.status.as_str(), row.duration_s, row.words), ("ended", Some(3.0), 3));
        assert!(s.audio(id).unwrap().is_some_and(|(d, exp)| d == "C:/r/1" && exp.is_some()));
        assert!(s.take_expired_audio().unwrap().is_empty());
        s.exec("UPDATE sessions SET audio_expires_at = '2000-01-01T00:00:00Z'");
        assert_eq!(s.take_expired_audio().unwrap(), vec!["C:/r/1".to_string()]);
        assert!(s.audio(id).unwrap().is_none());
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn busca_sem_acento_com_prefixo_e_sem_eco() {
        let (dir, mut s) = tmp("search");
        let w = |word: &str, start: f64, end: f64| Word { word: word.into(), start, end };
        let a = s.create_meeting("Planejamento", "0.0.1").unwrap();
        s.add_segment(a, "OUTROS", "A reunião de orçamento ficou para sexta", 0, &[w("A", 1.0, 1.1), w("reunião", 1.1, 1.5)]).unwrap();
        let echo = s.add_segment(a, "EU", "orçamento repetido", 0, &[w("orçamento", 2.0, 2.4)]).unwrap();
        s.set_segment_flag(echo, FLAG_ECHO, true).unwrap();
        let f = s.create_file_session("vídeo", Path::new(r"C:\v\a.mp4"), &transcript(), "0.0.1").unwrap();

        let hits = s.search("orcamento", 50).unwrap();
        assert_eq!(hits.len(), 1, "o eco fica de fora");
        assert_eq!((hits[0].session_id, hits[0].channel.as_str(), hits[0].start_s), (a, "OUTROS", 1.0));
        assert!(hits[0].snippet.contains(&format!("{HIT_START}orçamento{HIT_END}")), "{}", hits[0].snippet);
        assert_eq!(s.search("REUNIAO sex", 50).unwrap().len(), 1, "prefixo e maiúscula");
        assert!(s.search("reuniao domingo", 50).unwrap().is_empty(), "todos os termos");
        assert!(s.search("\"*:()", 50).unwrap().is_empty(), "pontuação não quebra a consulta");
        // Correção no editor atualiza o índice (trigger de UPDATE).
        assert_eq!(s.search("tudo", 50).unwrap()[0].session_id, f);
        s.update_word(f, 1, "Todo").unwrap();
        assert!(s.search("tudo", 50).unwrap().is_empty());
        assert_eq!(s.search("todo", 50).unwrap().len(), 1);
        // Apagar a sessão tira as falas do índice (o cascade dispara o trigger).
        s.delete_session(a).unwrap();
        assert!(s.search("orcamento", 50).unwrap().is_empty());
        assert_eq!(s.counts().unwrap(), (0, 1));
        s.set_audio_dir(f, "C:/r/9").unwrap();
        assert_eq!(s.delete_all().unwrap(), vec!["C:/r/9".to_string()]);
        assert_eq!(s.counts().unwrap(), (0, 0));
        assert!(s.search("todo", 50).unwrap().is_empty());
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn indice_de_busca_reconstruido_na_migracao() {
        let dir = std::env::temp_dir().join(format!("ouveai-store-mig-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("t.db");
        {
            // Banco da Fase 4 (user_version 3) com uma fala já gravada.
            let c = Connection::open(&db).unwrap();
            for sql in &MIGRATIONS[..3] {
                c.execute_batch(sql).unwrap();
            }
            c.pragma_update(None, "user_version", 3).unwrap();
            c.execute_batch(
                "INSERT INTO sessions (title, kind, status) VALUES ('r', 'meeting', 'ended');
                 INSERT INTO segments (session_id, channel, start_s, end_s, text) VALUES (1, 'EU', 0, 1, 'decisão tomada');",
            )
            .unwrap();
        }
        let s = Store::open(&db).unwrap();
        assert_eq!(s.search("decisao", 10).unwrap().len(), 1);
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sessao_orfa_vira_interrompida() {
        let (dir, s) = tmp("orphan");
        s.exec("INSERT INTO sessions (title, kind, status) VALUES ('r', 'meeting', 'recording'), ('p', 'meeting', 'paused'), ('e', 'meeting', 'ended');");
        let w = |word: &str, start: f64, end: f64| Word { word: word.into(), start, end };
        let mut s = s;
        s.add_segment(1, "EU", "oi", 0, &[w("oi", 1.0, 4.5)]).unwrap();
        s.set_audio_dir(1, "C:/r/1").unwrap();
        assert_eq!(s.recover_orphans(Some(7)).unwrap(), 2);
        let st: Vec<String> = s.sessions(None).unwrap().into_iter().map(|r| r.status).collect();
        assert_eq!(st.iter().filter(|x| *x == "interrupted").count(), 2);
        // A sessão que caiu fica com a duração até a última fala e o áudio com validade.
        assert_eq!(s.session(1).unwrap().unwrap().duration_s, Some(4.5));
        assert!(s.audio(1).unwrap().is_some_and(|(d, exp)| d == "C:/r/1" && exp.is_some()));
        assert_eq!(s.recover_orphans(Some(7)).unwrap(), 0);
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }
}
