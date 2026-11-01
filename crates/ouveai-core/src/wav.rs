//! WAV PCM16 mono gravado aos poucos (áudio da reunião, D2). O cabeçalho é
//! regravado a cada segundo, então o arquivo continua válido se o app cair.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{Duration, Instant};

pub struct Writer {
    w: BufWriter<File>,
    rate: u32,
    /// Posição (amostras desde o início da sessão) da primeira amostra.
    start: Option<u64>,
    written: u64,
    last_header: Instant,
}

impl Writer {
    pub fn create(path: &Path, rate: u32) -> std::io::Result<Self> {
        let mut w = BufWriter::new(File::create(path)?);
        w.write_all(&[0u8; 44])?;
        let mut s = Writer { w, rate, start: None, written: 0, last_header: Instant::now() };
        s.header()?;
        Ok(s)
    }

    fn header(&mut self) -> std::io::Result<()> {
        let data = (self.written * 2).min(u32::MAX as u64 - 36) as u32;
        let mut h = Vec::with_capacity(44);
        h.extend_from_slice(b"RIFF");
        h.extend_from_slice(&(36 + data).to_le_bytes());
        h.extend_from_slice(b"WAVEfmt ");
        h.extend_from_slice(&16u32.to_le_bytes());
        h.extend_from_slice(&1u16.to_le_bytes());
        h.extend_from_slice(&1u16.to_le_bytes());
        h.extend_from_slice(&self.rate.to_le_bytes());
        h.extend_from_slice(&(self.rate * 2).to_le_bytes());
        h.extend_from_slice(&2u16.to_le_bytes());
        h.extend_from_slice(&16u16.to_le_bytes());
        h.extend_from_slice(b"data");
        h.extend_from_slice(&data.to_le_bytes());
        self.w.flush()?;
        let f = self.w.get_mut();
        let end = f.seek(SeekFrom::End(0))?;
        f.seek(SeekFrom::Start(0))?;
        f.write_all(&h)?;
        f.seek(SeekFrom::Start(end))?;
        self.last_header = Instant::now();
        Ok(())
    }

    /// Escreve na posição da sessão. O que faltou (pausa) vira silêncio, para o
    /// tempo do arquivo bater com o da transcrição.
    pub fn put(&mut self, pos: u64, samples: &[f32]) -> std::io::Result<()> {
        let start = *self.start.get_or_insert(0);
        let at = pos.saturating_sub(start);
        let mut skip = 0usize;
        if at > self.written {
            let gap = at - self.written;
            for _ in 0..gap {
                self.w.write_all(&[0, 0])?;
            }
            self.written += gap;
        } else if at < self.written {
            skip = ((self.written - at) as usize).min(samples.len());
        }
        for &x in &samples[skip..] {
            self.w.write_all(&((x.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
        }
        self.written += (samples.len() - skip) as u64;
        if self.last_header.elapsed() >= Duration::from_secs(1) {
            self.header()?;
        }
        Ok(())
    }

    pub fn finish(mut self) -> std::io::Result<u64> {
        self.header()?;
        Ok(self.written)
    }
}

/// Lê [start_s, end_s) de um WAV gravado pelo `Writer` (PCM16 mono, cabeçalho de
/// 44 bytes). Usa o tamanho real do arquivo, não o do cabeçalho: se o app caiu,
/// o último segundo ainda é lido.
pub fn read_clip(path: &Path, rate: u32, start_s: f64, end_s: f64) -> std::io::Result<Vec<i16>> {
    use std::io::Read;
    let mut f = File::open(path)?;
    let total = (f.metadata()?.len().saturating_sub(44) / 2) as usize;
    let a = ((start_s.max(0.0) * rate as f64) as usize).min(total);
    let b = ((end_s.max(0.0) * rate as f64) as usize).clamp(a, total);
    f.seek(SeekFrom::Start(44 + a as u64 * 2))?;
    let mut buf = vec![0u8; (b - a) * 2];
    f.read_exact(&mut buf)?;
    Ok(buf.as_chunks::<2>().0.iter().map(|c| i16::from_le_bytes(*c)).collect())
}

/// Soma faixas (EU + OUTROS) com saturação. A mais curta é completada com silêncio.
pub fn mix(tracks: &[Vec<i16>]) -> Vec<i16> {
    let n = tracks.iter().map(Vec::len).max().unwrap_or(0);
    (0..n).map(|i| tracks.iter().map(|t| t.get(i).copied().unwrap_or(0) as i32).sum::<i32>().clamp(i16::MIN as i32, i16::MAX as i32) as i16).collect()
}

/// WAV PCM16 mono em memória (para tocar um trecho na interface).
pub fn encode(samples: &[i16], rate: u32) -> Vec<u8> {
    let data = (samples.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + data as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&rate.to_le_bytes());
    v.extend_from_slice(&(rate * 2).to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorte_mistura_e_codifica() {
        let dir = std::env::temp_dir().join(format!("ouveai-wav-clip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.wav");
        let mut w = Writer::create(&p, 100).unwrap();
        let ramp: Vec<f32> = (0..300).map(|i| i as f32 / 1000.0).collect();
        w.put(0, &ramp).unwrap();
        drop(w); // sem finish(): simula o app caindo (cabeçalho desatualizado)
        let c = read_clip(&p, 100, 1.0, 2.0).unwrap();
        assert_eq!(c.len(), 100);
        assert_eq!(c[0], (0.1f32 * 32767.0) as i16);
        assert_eq!(read_clip(&p, 100, 2.5, 9.0).unwrap().len(), 50, "corta no fim do arquivo");
        assert!(read_clip(&p, 100, 5.0, 6.0).unwrap().is_empty());
        let m = mix(&[vec![30000, 1, 2], vec![30000]]);
        assert_eq!(m, vec![i16::MAX, 1, 2]);
        let e = encode(&m, 16_000);
        assert_eq!((e.len(), &e[..4], u32::from_le_bytes([e[40], e[41], e[42], e[43]])), (50, &b"RIFF"[..], 6));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn lacuna_vira_silencio_e_cabecalho_fecha() {
        let dir = std::env::temp_dir().join(format!("ouveai-wav-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.wav");
        let mut w = Writer::create(&p, 16_000).unwrap();
        w.put(0, &[0.5; 100]).unwrap();
        w.put(300, &[0.5; 100]).unwrap(); // pausa de 200 amostras
        assert_eq!(w.finish().unwrap(), 400);
        let b = std::fs::read(&p).unwrap();
        assert_eq!(b.len(), 44 + 800);
        assert_eq!(u32::from_le_bytes([b[40], b[41], b[42], b[43]]), 800);
        assert_eq!(&b[44 + 300..44 + 302], &[0, 0]);
        std::fs::remove_dir_all(&dir).ok();
    }
}
