//! Linha do tempo de um canal (6.3 do plano): cada pacote vai para a posição
//! do seu QPC, as lacunas viram silêncio e as fontes do mesmo canal (loopback
//! multimídia + comunicação) são somadas, sem duplicar.
//!
//! Tolerância (S3, f1 × f2): se o QPC do pacote estiver a até `tol` amostras
//! de onde a fonte parou, a faixa segue contínua pela contagem de amostras.
//! Só um desvio maior vira lacuna (preenchida) ou sobreposição (cortada).

use std::collections::VecDeque;

pub const MAX_SOURCES: usize = 4;

/// Uma lacuna maior que isso (10 min a 16 kHz) não é silêncio de verdade, é um
/// tempo de pacote absurdo: a fonte segue pela contagem de amostras em vez de
/// reservar memória para a lacuna.
pub const MAX_GAP: u64 = 16_000 * 600;

pub struct Timeline {
    /// Posição (amostras desde o início da sessão) de `buf[0]`.
    base: u64,
    buf: VecDeque<f32>,
    cursors: [Option<u64>; MAX_SOURCES],
    tol: u64,
}

impl Timeline {
    pub fn new(start: u64, tol: u64) -> Self {
        Timeline { base: start, buf: VecDeque::new(), cursors: [None; MAX_SOURCES], tol }
    }

    pub fn base(&self) -> u64 {
        self.base
    }

    /// Fonte reaberta (outro dispositivo): o próximo pacote vale pelo QPC.
    pub fn reset_source(&mut self, src: usize) {
        self.cursors[src] = None;
    }

    /// Soma `samples` na linha do tempo. `pos = None` = sem QPC confiável
    /// (anexa onde a fonte parou). Devolve o tamanho da lacuna (em amostras)
    /// quando o QPC pulou além da tolerância.
    pub fn put(&mut self, src: usize, pos: Option<u64>, samples: &[f32]) -> Option<u64> {
        let end = self.base + self.buf.len() as u64;
        let pos = pos.filter(|&p| p <= self.cursors[src].unwrap_or(end).max(end) + MAX_GAP);
        let mut skip = 0usize;
        let mut gap = None;
        let p = match (pos, self.cursors[src]) {
            (Some(p), Some(c)) if p.abs_diff(c) <= self.tol => c,
            (Some(p), Some(c)) if p < c => {
                // Sobreposição real: corta o que já foi escrito por esta fonte.
                skip = ((c - p) as usize).min(samples.len());
                c
            }
            (Some(p), Some(c)) => {
                gap = Some(p - c);
                p
            }
            (Some(p), None) => p,
            // Sem tempo confiável e a fonte ficou para trás do que já foi entregue
            // (ex.: o dispositivo ficou sem mandar pacotes enquanto nada tocava):
            // retoma no primeiro ponto ainda não entregue, em vez de descartar tudo.
            (None, Some(c)) => c.max(self.base),
            (None, None) => self.base + self.buf.len() as u64,
        };
        let len = (samples.len() - skip) as u64;
        self.cursors[src] = Some(p + len);
        // Chegou tarde demais (já entregue): descarta a parte antiga.
        let (mut p, mut s) = (p, &samples[skip..]);
        if p < self.base {
            let late = ((self.base - p) as usize).min(s.len());
            s = &s[late..];
            p = self.base;
        }
        let off = (p - self.base) as usize;
        if self.buf.len() < off + s.len() {
            self.buf.resize(off + s.len(), 0.0);
        }
        for (i, &x) in s.iter().enumerate() {
            self.buf[off + i] += x;
        }
        gap
    }

    /// Entrega tudo antes de `upto` (o que não chegou é silêncio).
    pub fn drain(&mut self, upto: u64) -> Option<(u64, Vec<f32>)> {
        if upto <= self.base {
            return None;
        }
        let n = (upto - self.base) as usize;
        if self.buf.len() < n {
            self.buf.resize(n, 0.0);
        }
        let out: Vec<f32> = self.buf.drain(..n).map(|x| x.clamp(-1.0, 1.0)).collect();
        let start = self.base;
        self.base = upto;
        Some((start, out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_dentro_da_tolerancia_fica_continuo() {
        let mut t = Timeline::new(0, 10);
        t.put(0, Some(0), &[1.0; 100]);
        // QPC diz 105 (jitter de 5): continua em 100, sem lacuna.
        assert_eq!(t.put(0, Some(105), &[0.5; 50]), None);
        let (s, v) = t.drain(150).unwrap();
        assert_eq!((s, v.len(), v[99], v[100], v[149]), (0, 150, 1.0, 0.5, 0.5));
    }

    #[test]
    fn lacuna_vira_silencio_e_loopback_mudo_tambem() {
        let mut t = Timeline::new(0, 10);
        t.put(0, Some(0), &[1.0; 10]);
        assert_eq!(t.put(0, Some(60), &[1.0; 10]), Some(50));
        // Nada chegou depois de 70: o drain completa com zeros até 100.
        let (_, v) = t.drain(100).unwrap();
        assert_eq!(v.len(), 100);
        assert_eq!(v[10..60].iter().sum::<f32>(), 0.0);
        assert_eq!(v[60], 1.0);
        assert_eq!(v[70..].iter().sum::<f32>(), 0.0);
        assert_eq!(t.base(), 100);
    }

    #[test]
    fn duas_fontes_somam_e_atraso_e_cortado() {
        let mut t = Timeline::new(1000, 10);
        t.put(0, Some(1000), &[0.25; 20]);
        t.put(1, Some(1010), &[0.5; 20]);
        let (s, v) = t.drain(1020).unwrap();
        assert_eq!(s, 1000);
        assert_eq!((v[5], v[15]), (0.25, 0.75));
        // Pacote da fonte 1 que começa antes do já entregue: só a parte nova entra.
        t.reset_source(1);
        t.put(1, Some(1015), &[0.5; 10]);
        let (_, v) = t.drain(1030).unwrap();
        assert_eq!(v.len(), 10);
        assert_eq!((v[0], v[4], v[5]), (1.0, 1.0, 0.5)); // 1020..1025: 0,5 novo + 0,5 que já estava
    }

    #[test]
    fn tempo_absurdo_segue_pela_contagem_de_amostras() {
        let mut t = Timeline::new(0, 10);
        t.put(0, Some(0), &[1.0; 100]);
        // 1,8 × 10¹² amostras à frente (o que derrubou o app no Sandbox): sem lacuna, sem alocar.
        assert_eq!(t.put(0, Some(1_842_513_589_741), &[0.5; 50]), None);
        let (s, v) = t.drain(150).unwrap();
        assert_eq!((s, v.len(), v[99], v[100]), (0, 150, 1.0, 0.5));
        // Primeiro pacote de uma fonte já vindo absurdo também não reserva memória.
        assert_eq!(t.put(1, Some(u64::MAX / 2), &[0.25; 10]), None);
        assert!(t.drain(160).is_some_and(|(_, v)| v.len() == 10));
    }

    #[test]
    fn fonte_sem_relogio_que_ficou_para_tras_nao_e_descartada() {
        let mut t = Timeline::new(0, 10);
        t.put(0, None, &[0.1; 10]);
        // O relógio da sessão andou (silêncio entregue) e a fonte ficou parada em 10.
        assert!(t.drain(1000).is_some());
        // Chega fala sem tempo confiável: entra em 1000, não é jogada fora.
        t.put(0, None, &[0.5; 50]);
        let (s, v) = t.drain(1050).unwrap();
        assert_eq!((s, v.len(), v[0], v[49]), (1000, 50, 0.5, 0.5));
        // E continua contínua dali.
        t.put(0, None, &[0.7; 10]);
        assert_eq!(t.drain(1060).unwrap().1[0], 0.7);
    }

    #[test]
    fn sobreposicao_alem_da_tolerancia_e_cortada() {
        let mut t = Timeline::new(0, 10);
        t.put(0, Some(0), &[1.0; 100]);
        t.put(0, Some(50), &[0.5; 100]); // 50 já escritas por esta fonte → só as 50 últimas entram, em 100
        let (_, v) = t.drain(150).unwrap();
        assert_eq!((v[60], v[120]), (1.0, 0.5));
    }
}
