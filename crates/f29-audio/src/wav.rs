//! Minimales WAV-Format (PCM): Schreiben für Hörproben, Lesen für eigene
//! Ansage-Samples (z. B. `assets/sounds/stall.wav`).

/// Schreibt Mono-Samples (−1..1) als 16-Bit-PCM-WAV.
pub fn write_mono16(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1_u16.to_le_bytes()); // mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    out.extend_from_slice(&2_u16.to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32_767.0).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Liest ein PCM-WAV (8 oder 16 Bit, mono oder mehrkanalig) als Mono-Samples
/// und rechnet es linear auf `target_rate` um. `None` bei anderem Format.
pub fn read_mono(bytes: &[u8], target_rate: u32) -> Option<Vec<f32>> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let u16_at = |p: usize| Some(u16::from_le_bytes(bytes.get(p..p + 2)?.try_into().ok()?));
    let u32_at = |p: usize| Some(u32::from_le_bytes(bytes.get(p..p + 4)?.try_into().ok()?));
    let (mut fmt, mut data) = (None, None);
    let mut pos = 12;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let len = u32_at(pos + 4)? as usize;
        let body = pos + 8;
        match id {
            b"fmt " => fmt = Some((u16_at(body)?, u16_at(body + 2)?, u32_at(body + 4)?, u16_at(body + 14)?)),
            b"data" => data = Some(bytes.get(body..(body + len).min(bytes.len()))?),
            _ => {}
        }
        pos = body + len + (len & 1);
    }
    let (format, channels, rate, bits) = fmt?;
    let data = data?;
    if format != 1 || channels == 0 || rate == 0 || !(bits == 8 || bits == 16) {
        return None;
    }
    let ch = usize::from(channels);
    let width = usize::from(bits / 8);
    let frames = data.len() / (ch * width);
    let sample = |i: usize| -> f32 {
        let p = i * width;
        if bits == 8 {
            (f32::from(data[p]) - 128.0) / 128.0
        } else {
            f32::from(i16::from_le_bytes([data[p], data[p + 1]])) / 32_768.0
        }
    };
    let mono: Vec<f32> =
        (0..frames).map(|f| (0..ch).map(|c| sample(f * ch + c)).sum::<f32>() / ch as f32).collect();
    if rate == target_rate {
        return Some(mono);
    }
    let ratio = rate as f32 / target_rate as f32;
    let n = (mono.len() as f32 / ratio) as usize;
    Some(
        (0..n)
            .map(|i| {
                let x = i as f32 * ratio;
                let k = x as usize;
                let t = x - k as f32;
                let a = mono[k.min(mono.len() - 1)];
                let b = mono[(k + 1).min(mono.len() - 1)];
                a + (b - a) * t
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_resample() {
        let s: Vec<f32> = (0..1_000).map(|i| ((i as f32) * 0.05).sin() * 0.5).collect();
        let bytes = write_mono16(&s, 22_050);
        let back = read_mono(&bytes, 22_050).expect("lesbar");
        assert_eq!(back.len(), s.len());
        assert!(s.iter().zip(&back).all(|(a, b)| (a - b).abs() < 1e-3));
        let up = read_mono(&bytes, 44_100).expect("umgerechnet");
        assert!((up.len() as i64 - 2_000).abs() <= 2);
        assert!(read_mono(b"kein wav", 44_100).is_none());
    }
}
