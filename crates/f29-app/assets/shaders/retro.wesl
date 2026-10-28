// Retro-Nachbearbeitung von F-29 Silent Sniper.
//
// 1. Optional: Pixelierung auf eine *virtuelle* Zeilenzahl (z. B. 360). Die
//    Blockgröße hängt von der Fensterhöhe ab – der Look ist damit in jeder
//    Auflösung gleich, statt an 320×200 gebunden zu sein.
// 2. Geordnetes Dithering (Bayer 4×4) im selben virtuellen Raster.
// 3. Quantisierung auf die 16-Farben-Palette (identisch mit
//    crates/f29-render/src/palette.rs – ein Test prüft das).

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct RetroEffect {
    // Virtuelle Zeilenzahl des Rasters (Fensterhöhe / Blockgröße).
    virtual_lines: f32,
    // Stärke des Ditherings (0 = aus, 1 = eine Palettenstufe).
    dither: f32,
    // 1 = auf Palette quantisieren, 0 = Originalfarben.
    palette: f32,
    // 1 = auf das virtuelle Raster pixelieren.
    pixelate: f32,
}
@group(0) @binding(2) var<uniform> settings: RetroEffect;

const PALETTE: array<vec3<f32>, 16> = array<vec3<f32>, 16>(
    vec3<f32>(0.0431, 0.0510, 0.0902),
    vec3<f32>(0.1137, 0.1686, 0.3255),
    vec3<f32>(0.1843, 0.3647, 0.6196),
    vec3<f32>(0.3098, 0.5608, 0.8471),
    vec3<f32>(0.5608, 0.7569, 0.9255),
    vec3<f32>(0.7882, 0.8745, 0.9412),
    vec3<f32>(0.9490, 0.9569, 0.9647),
    vec3<f32>(0.1373, 0.2510, 0.1216),
    vec3<f32>(0.2471, 0.4353, 0.1725),
    vec3<f32>(0.4353, 0.6039, 0.2353),
    vec3<f32>(0.8431, 0.7686, 0.5490),
    vec3<f32>(0.5412, 0.4353, 0.2902),
    vec3<f32>(0.3569, 0.2902, 0.2275),
    vec3<f32>(0.4902, 0.4980, 0.5255),
    vec3<f32>(0.6902, 0.7020, 0.7294),
    vec3<f32>(0.6588, 0.2627, 0.1843)
);

const BAYER: array<f32, 16> = array<f32, 16>(
    0.0, 8.0, 2.0, 10.0,
    12.0, 4.0, 14.0, 6.0,
    3.0, 11.0, 1.0, 9.0,
    15.0, 7.0, 13.0, 5.0,
);

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn nearest_palette(c: vec3<f32>) -> vec3<f32> {
    var best = PALETTE[0];
    var best_d = 1e9;
    for (var i = 0u; i < 16u; i = i + 1u) {
        let d3 = c - PALETTE[i];
        // Grün etwas stärker gewichten (Helligkeitswahrnehmung)
        let d = dot(d3 * d3, vec3<f32>(0.30, 0.59, 0.11));
        if (d < best_d) {
            best_d = d;
            best = PALETTE[i];
        }
    }
    return best;
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(screen_texture));
    // Blockgröße in physischen Pixeln (mindestens 1)
    let block = max(1.0, size.y / max(settings.virtual_lines, 1.0));
    let cell = floor(in.position.xy / block);

    var uv = in.uv;
    if (settings.pixelate > 0.5) {
        uv = (cell + 0.5) * block / size;
    }
    let linear = textureSample(screen_texture, texture_sampler, uv).rgb;
    if (settings.palette < 0.5) {
        return vec4<f32>(linear, 1.0);
    }

    var srgb = linear_to_srgb(linear);
    let bx = u32(cell.x) % 4u;
    let by = u32(cell.y) % 4u;
    let threshold = (BAYER[by * 4u + bx] + 0.5) / 16.0 - 0.5;
    // Eine Palettenstufe entspricht grob 1/6 des Wertebereichs
    srgb = srgb + vec3<f32>(threshold * settings.dither / 6.0);
    let q = nearest_palette(clamp(srgb, vec3<f32>(0.0), vec3<f32>(1.0)));
    return vec4<f32>(srgb_to_linear(q), 1.0);
}
