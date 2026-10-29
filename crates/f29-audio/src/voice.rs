//! Kleine Formant-Sprachsynthese für Ansagen im Retro-Stil („STALL“).
//!
//! Ein Wort ist eine Folge von Lautsegmenten. Stimmhafte Laute entstehen aus
//! einer Sägezahn-Stimmquelle durch drei in Reihe geschaltete Formant-
//! Resonatoren, Zischlaute aus gefiltertem Rauschen. Zwischen den Segmenten
//! werden Formanten und Pegel weich übergeblendet. Zum Schluss wird das
//! Ergebnis grob abgetastet und quantisiert – das gibt den typischen
//! Klang früher Sprachchips, ohne fremde Aufnahmen.

use crate::dsp::{Noise, OnePole, Phase, Resonator, saw};

/// Ein Lautsegment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    /// Dauer in Sekunden.
    pub dur: f32,
    /// Formantfrequenzen F1–F3 (Hz).
    pub f: [f32; 3],
    /// Bandbreiten der Formanten (Hz).
    pub bw: [f32; 3],
    /// Pegel der Stimmquelle 0..1.
    pub voiced: f32,
    /// Pegel des Rauschens (Zisch-/Explosivlaut) 0..1.
    pub noise: f32,
    /// Mittenfrequenz des Rauschens (Hz).
    pub noise_f: f32,
    /// Grundfrequenz am Ende des Segments (Hz).
    pub pitch: f32,
}

const NEUTRAL_F: [f32; 3] = [500.0, 1_500.0, 2_500.0];
const NEUTRAL_BW: [f32; 3] = [90.0, 110.0, 160.0];

const fn silence(dur: f32, pitch: f32) -> Segment {
    Segment { dur, f: NEUTRAL_F, bw: NEUTRAL_BW, voiced: 0.0, noise: 0.0, noise_f: 3_000.0, pitch }
}

/// Das Wort „STALL“ (S – T – AO – L).
pub const STALL: [Segment; 7] = [
    silence(0.03, 130.0),
    // S: scharfes Zischen
    Segment {
        dur: 0.15,
        f: [500.0, 1_700.0, 2_600.0],
        bw: NEUTRAL_BW,
        voiced: 0.0,
        noise: 0.9,
        noise_f: 5_200.0,
        pitch: 130.0,
    },
    // T: Verschluss, dann Sprengung
    silence(0.06, 135.0),
    Segment {
        dur: 0.03,
        f: [500.0, 1_700.0, 2_600.0],
        bw: NEUTRAL_BW,
        voiced: 0.0,
        noise: 0.8,
        noise_f: 3_800.0,
        pitch: 140.0,
    },
    // AO: offenes „o“, fallende Tonhöhe
    Segment {
        dur: 0.26,
        f: [600.0, 880.0, 2_450.0],
        bw: [80.0, 90.0, 140.0],
        voiced: 1.0,
        noise: 0.03,
        noise_f: 1_500.0,
        pitch: 112.0,
    },
    // L
    Segment {
        dur: 0.16,
        f: [380.0, 1_050.0, 2_700.0],
        bw: [100.0, 150.0, 220.0],
        voiced: 0.75,
        noise: 0.0,
        noise_f: 1_500.0,
        pitch: 100.0,
    },
    silence(0.07, 95.0),
];

/// „PULL UP“ (P – UH – L – AH – P), dringlich und etwas höher gesprochen.
pub const PULL_UP: [Segment; 9] = [
    silence(0.03, 150.0),
    // P: Sprengung
    Segment {
        dur: 0.03,
        f: [400.0, 1_000.0, 2_400.0],
        bw: NEUTRAL_BW,
        voiced: 0.0,
        noise: 0.7,
        noise_f: 1_200.0,
        pitch: 150.0,
    },
    // UH (wie in „pull“)
    Segment {
        dur: 0.13,
        f: [450.0, 1_030.0, 2_380.0],
        bw: [80.0, 100.0, 140.0],
        voiced: 1.0,
        noise: 0.02,
        noise_f: 1_500.0,
        pitch: 140.0,
    },
    // L
    Segment {
        dur: 0.12,
        f: [380.0, 1_000.0, 2_700.0],
        bw: [100.0, 150.0, 220.0],
        voiced: 0.8,
        noise: 0.0,
        noise_f: 1_500.0,
        pitch: 130.0,
    },
    silence(0.04, 150.0),
    // AH (wie in „up“)
    Segment {
        dur: 0.15,
        f: [640.0, 1_190.0, 2_390.0],
        bw: [80.0, 100.0, 140.0],
        voiced: 1.0,
        noise: 0.02,
        noise_f: 1_500.0,
        pitch: 135.0,
    },
    // P: Verschluss und leichte Sprengung
    silence(0.06, 120.0),
    Segment {
        dur: 0.03,
        f: [400.0, 1_000.0, 2_400.0],
        bw: NEUTRAL_BW,
        voiced: 0.0,
        noise: 0.4,
        noise_f: 1_200.0,
        pitch: 120.0,
    },
    silence(0.06, 110.0),
];

/// Überblendzeit zwischen Segmenten (s).
const TRANSITION: f32 = 0.03;
/// Abtastrate des „Sprachchips“ (Hz) und Auflösung der Quantisierung.
const CHIP_RATE: f32 = 11_025.0;
const CHIP_LEVELS: f32 = 127.0;

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Synthetisiert ein Wort bei Abtastrate `sr`; Spitzenpegel 0,9.
pub fn speak(segments: &[Segment], sr: u32) -> Vec<f32> {
    let sr_f = sr as f32;
    let total: f32 = segments.iter().map(|s| s.dur).sum();
    let n = (total * sr_f) as usize;
    let mut out = Vec::with_capacity(n);
    let (mut r1, mut r2, mut r3, mut rn) =
        (Resonator::default(), Resonator::default(), Resonator::default(), Resonator::default());
    let mut glottis = Phase::default();
    let mut glottal_lp = OnePole::default();
    let mut noise = Noise::new(0x5741_4C4C);
    let (mut seg, mut seg_start) = (0usize, 0.0_f32);
    let mut hold = (0.0_f32, 0.0_f32);
    let a_glottal = OnePole::coeff(2_000.0, sr_f);

    for i in 0..n {
        let t = i as f32 / sr_f;
        while seg + 1 < segments.len() && t >= seg_start + segments[seg].dur {
            seg_start += segments[seg].dur;
            seg += 1;
        }
        let cur = segments[seg];
        let prev = if seg > 0 { segments[seg - 1] } else { cur };
        let k = ((t - seg_start) / TRANSITION).clamp(0.0, 1.0);
        let pitch = lerp(prev.pitch, cur.pitch, ((t - seg_start) / cur.dur).clamp(0.0, 1.0));
        let f = [0, 1, 2].map(|j| lerp(prev.f[j], cur.f[j], k));
        let bw = [0, 1, 2].map(|j| lerp(prev.bw[j], cur.bw[j], k));
        let voiced = lerp(prev.voiced, cur.voiced, k);
        let noise_amp = lerp(prev.noise, cur.noise, k);
        let noise_f = lerp(prev.noise_f, cur.noise_f, k);

        // Stimmquelle: weicher Sägezahn, Rauschquelle: Bandpass
        let src = glottal_lp.process(saw(glottis.advance(pitch, sr_f)), a_glottal) * voiced;
        let mut y =
            r3.process(r2.process(r1.process(src, f[0], bw[0], sr_f), f[1], bw[1], sr_f), f[2], bw[2], sr_f);
        y += rn.process(noise.sample(), noise_f, noise_f * 0.35, sr_f) * noise_amp * 0.6;

        // Sprachchip: grob abtasten und quantisieren
        hold.1 += CHIP_RATE / sr_f;
        if hold.1 >= 1.0 {
            hold.1 -= 1.0;
            hold.0 = y;
        }
        out.push(hold.0);
    }
    normalize(&mut out, 0.9);
    for v in &mut out {
        *v = (*v * CHIP_LEVELS).round() / CHIP_LEVELS;
    }
    // Leichtes Glätten gegen harte Treppenstufen
    let mut lp = OnePole::default();
    let a = OnePole::coeff(5_500.0, sr_f);
    for v in &mut out {
        *v = lp.process(*v, a);
    }
    normalize(&mut out, 0.9);
    out
}

fn normalize(buf: &mut [f32], peak: f32) {
    let max = buf.iter().fold(0.0_f32, |m, v| m.max(v.abs()));
    if max > 1e-9 {
        let g = peak / max;
        for v in buf {
            *v *= g;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pull_up_is_short_and_bounded() {
        let w = speak(&PULL_UP, 44_100);
        let dur = w.len() as f32 / 44_100.0;
        assert!((0.5..0.8).contains(&dur), "Dauer {dur}");
        assert!(w.iter().all(|v| v.is_finite() && v.abs() <= 0.91));
        let rms = (w.iter().map(|v| v * v).sum::<f32>() / w.len() as f32).sqrt();
        assert!(rms > 0.08, "zu leise: {rms}");
    }

    #[test]
    fn stall_word_has_hiss_then_voice() {
        let sr = 44_100;
        let w = speak(&STALL, sr);
        let dur = w.len() as f32 / sr as f32;
        assert!((0.6..0.9).contains(&dur), "Dauer {dur}");
        assert!(w.iter().all(|v| v.is_finite() && v.abs() <= 0.91));
        // Energie und Nulldurchgänge in Fenstern: S ist zischend (viele
        // Nulldurchgänge), der Vokal tieffrequent (wenige).
        let window = |from: f32, to: f32| {
            let s = &w[(from * sr as f32) as usize..(to * sr as f32) as usize];
            let rms = (s.iter().map(|v| v * v).sum::<f32>() / s.len() as f32).sqrt();
            let zc = s.windows(2).filter(|p| (p[0] < 0.0) != (p[1] < 0.0)).count() as f32 / (to - from);
            (rms, zc)
        };
        let (s_rms, s_zc) = window(0.06, 0.16);
        let (v_rms, v_zc) = window(0.35, 0.5);
        assert!(s_rms > 0.03 && v_rms > 0.1, "S {s_rms}, Vokal {v_rms}");
        assert!(s_zc > 2.0 * v_zc, "Nulldurchgänge S {s_zc} vs Vokal {v_zc}");
    }
}
