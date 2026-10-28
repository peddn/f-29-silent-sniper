//! Die 16-Farben-Palette von F-29 Silent Sniper.
//!
//! Eigene Palette, abgestimmt auf Gelände, Himmel und Dunst. Der
//! Post-Processing-Shader des Frontends quantisiert das Bild auf genau diese
//! Farben (mit geordnetem Dithering). Die Shader-Datei
//! `assets/shaders/retro.wesl` enthält dieselben Werte; ein Test prüft, dass
//! beide übereinstimmen.

/// sRGB-Farben als `0xRRGGBB`.
pub const PALETTE: [u32; 16] = [
    0x0b0d17, // 0  Nachtschwarz
    0x1d2b53, // 1  Tiefblau
    0x2f5d9e, // 2  Wasser
    0x4f8fd8, // 3  Himmel
    0x8fc1ec, // 4  Himmel hell
    0xc9dff0, // 5  Dunst
    0xf2f4f6, // 6  Schnee
    0x23401f, // 7  Wald
    0x3f6f2c, // 8  Wiese
    0x6f9a3c, // 9  Wiese hell
    0xd7c48c, // 10 Sand
    0x8a6f4a, // 11 Erde
    0x5b4a3a, // 12 Erde dunkel
    0x7d7f86, // 13 Fels
    0xb0b3ba, // 14 Fels hell
    0xa8432f, // 15 Rost/Signal
];

/// Palettenfarbe als sRGB-Tripel 0..1.
pub fn srgb(index: usize) -> [f32; 3] {
    let c = PALETTE[index];
    [((c >> 16) & 0xff) as f32 / 255.0, ((c >> 8) & 0xff) as f32 / 255.0, (c & 0xff) as f32 / 255.0]
}

/// Mischt zwei Palettenfarben (für Zwischentöne vor dem Dithering).
pub fn mix(a: usize, b: usize, t: f32) -> [f32; 3] {
    let (x, y) = (srgb(a), srgb(b));
    [x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t, x[2] + (y[2] - x[2]) * t]
}

pub const SKY: usize = 3;
pub const HAZE: usize = 5;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_palette_matches() {
        let shader = include_str!("../../f29-app/assets/shaders/retro.wesl");
        for (i, c) in PALETTE.iter().enumerate() {
            let [r, g, b] = srgb(i);
            let needle = format!("vec3<f32>({r:.4}, {g:.4}, {b:.4})");
            assert!(
                shader.contains(&needle),
                "Palettenfarbe {i} (#{c:06x}) fehlt im Shader: erwartet `{needle}`"
            );
        }
    }
}
