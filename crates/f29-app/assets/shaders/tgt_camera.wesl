// Zielkamera der TGT-Seite: macht aus dem gerenderten Ausschnitt ein
// körniges Schwarzweiß-Kamerabild mit leicht grünlicher Tönung.
//
// 1. Helligkeit, Kontrast angehoben (frühe TV-Kamera, wenig Grauwerte).
// 2. Wanderndes Rauschen je Bildpunkt und schwach flackernde Zeilen.
// 3. Zeilenstruktur und abgedunkelte Ränder.

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct TgtCameraEffect {
    time: f32,
    noise: f32,
    lines: f32,
    _pad: f32,
}
@group(0) @binding(2) var<uniform> settings: TgtCameraEffect;

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

fn linear_to_srgb(c: f32) -> f32 {
    return select(1.055 * pow(max(c, 0.0), 1.0 / 2.4) - 0.055, c * 12.92, c <= 0.0031308);
}

fn srgb_to_linear(c: f32) -> f32 {
    return select(pow((c + 0.055) / 1.055, 2.4), c / 12.92, c <= 0.04045);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(screen_texture));
    let px = floor(in.uv * size);
    let frame = floor(settings.time * 24.0);

    // Zeilen versetzen sich gelegentlich ein wenig (Synchronisationsfehler).
    let row_jitter = (hash(vec2<f32>(px.y, frame)) - 0.5) * step(0.97, hash(vec2<f32>(frame, 7.0)));
    let uv = in.uv + vec2<f32>(row_jitter * 2.0 / size.x, 0.0);
    let rgb = textureSample(screen_texture, texture_sampler, uv).rgb;

    var lum = linear_to_srgb(dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722)));
    lum = clamp((lum - 0.5) * 1.5 + 0.5, 0.0, 1.0);

    // Rauschen: je Bild neu, Stärke aus der Entfernung.
    let grain = hash(px + vec2<f32>(frame * 17.0, frame * 31.0)) - 0.5;
    let row = hash(vec2<f32>(px.y * 0.37, frame)) - 0.5;
    lum = lum + (grain * 0.22 + row * 0.05) * settings.noise;

    // Zeilenstruktur und Vignette.
    let scan = 0.82 + 0.18 * step(0.5, fract(px.y * 0.5));
    let d = in.uv - vec2<f32>(0.5);
    let vignette = 1.0 - dot(d, d) * 1.1;
    lum = clamp(lum * scan * vignette, 0.0, 1.0);

    let tint = vec3<f32>(0.80, 1.0, 0.82) * lum;
    return vec4<f32>(srgb_to_linear(tint.r), srgb_to_linear(tint.g), srgb_to_linear(tint.b), 1.0);
}
