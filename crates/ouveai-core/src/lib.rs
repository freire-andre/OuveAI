//! Núcleo do OuveAI (sem Tauri, sem WASAPI).

pub mod asr;
pub mod diag;
pub mod export;
pub mod hash;
pub mod live;
pub mod media;
pub mod models;
pub mod net;
pub mod paths;
pub mod store;
pub mod subs;
pub mod suspects;
pub mod sys;
pub mod transcribe;
pub mod update;
pub mod wav;
pub mod words;

/// Testes que mexem em variáveis de ambiente (`OUVEAI_DATA_DIR`) rodam um de cada vez.
#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
