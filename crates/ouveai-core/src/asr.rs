//! Reconhecedor offline (Parakeet TDT 0.6b v3 int8 via `sherpa-onnx`).
//! Campos e nomes conferidos no crate 2.0.1 e no S1 (regra 5).

use crate::models::{self, Presence};
use crate::words::Tokens;
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig};

pub const SAMPLE_RATE: i32 = 16_000;

/// O encoder em ONNX aceita até 5000 quadros de 80 ms (400 s); acima disso o
/// onnxruntime aborta o processo inteiro (exceção nativa, não capturável).
/// Trava com folga: nenhum trecho maior que isso chega ao modelo.
pub const MAX_SECONDS: f64 = 390.0;

pub struct Asr {
    rec: OfflineRecognizer,
}

/// `min(4, núcleos)` para arquivo (plano, Fase 3).
pub fn file_threads() -> i32 {
    std::thread::available_parallelism().map(|n| n.get().min(4) as i32).unwrap_or(4)
}

impl Asr {
    /// Carrega o Parakeet da pasta de modelos. Leva ~3 s e ~700 MB de RAM.
    pub fn load(threads: i32) -> Result<Self, String> {
        let spec = models::parakeet();
        match models::presence(&spec) {
            Presence::Ready => {}
            Presence::Absent => return Err("o modelo de reconhecimento de fala não está instalado (veja Modelos)".into()),
            Presence::Corrupt(d) => return Err(format!("o modelo de reconhecimento está incompleto ({d}); baixe ou importe de novo")),
        }
        let f = |name: &str| Some(spec.file(name).to_string_lossy().into_owned());
        let mut cfg = OfflineRecognizerConfig::default();
        cfg.model_config.transducer = OfflineTransducerModelConfig {
            encoder: f("encoder.int8.onnx"),
            decoder: f("decoder.int8.onnx"),
            joiner: f("joiner.int8.onnx"),
        };
        cfg.model_config.tokens = f("tokens.txt");
        cfg.model_config.model_type = Some("nemo_transducer".into());
        cfg.model_config.num_threads = threads;
        cfg.model_config.provider = Some("cpu".into());
        // S1: beam search não aceita hotwords neste modelo e omitiu frases; greedy.
        cfg.decoding_method = Some("greedy_search".into());
        let rec = OfflineRecognizer::create(&cfg).ok_or("falha ao carregar o modelo de reconhecimento")?;
        Ok(Self { rec })
    }

    /// Reconhece um trecho de amostras 16 kHz mono. Tempos relativos ao trecho.
    pub fn recognize(&self, samples: &[f32]) -> Result<Tokens, String> {
        let secs = samples.len() as f64 / SAMPLE_RATE as f64;
        if secs > MAX_SECONDS {
            return Err(format!("trecho de {secs:.0} s passa do limite do modelo ({MAX_SECONDS:.0} s)"));
        }
        let stream = self.rec.create_stream();
        stream.accept_waveform(SAMPLE_RATE, samples);
        self.rec.decode(&stream);
        Ok(match stream.get_result() {
            Some(r) => Tokens { timestamps: r.timestamps.unwrap_or_default(), durations: r.durations, tokens: r.tokens },
            None => Tokens::default(),
        })
    }
}
