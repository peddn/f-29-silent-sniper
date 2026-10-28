//! Retro-Nachbearbeitung: Palette, Dithering, optionale Pixelierung.
//!
//! Siehe `assets/shaders/retro.wesl`. Der Look ist auflösungsunabhängig: Das
//! Dither-/Pixelraster bezieht sich auf eine *virtuelle* Zeilenzahl, die mit
//! der Fensterhöhe skaliert.

use bevy::{
    core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin},
    prelude::*,
    render::{RenderApp, extract_component::ExtractComponent, render_resource::ShaderType},
    shader::ShaderRef,
};

pub struct RetroPlugin;

impl Plugin for RetroPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FullscreenMaterialPlugin::<RetroEffect>::default());
    }
}

/// Stufen des Retro-Looks (F3 schaltet weiter).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetroMode {
    /// Palette, Dithering und grobes Raster.
    Full,
    /// Palette und Dithering in voller Auflösung.
    Palette,
    /// Unbearbeitete Farben.
    Off,
}

impl RetroMode {
    pub fn next(self) -> Self {
        match self {
            RetroMode::Full => RetroMode::Palette,
            RetroMode::Palette => RetroMode::Off,
            RetroMode::Off => RetroMode::Full,
        }
    }
}

/// Uniform-Daten des Shaders (16 Byte – keine WebGL2-Polsterung nötig).
#[derive(Component, ExtractComponent, Clone, Copy, ShaderType, Default, Debug, PartialEq)]
#[extract_app(RenderApp)]
pub struct RetroEffect {
    pub virtual_lines: f32,
    pub dither: f32,
    pub palette: f32,
    pub pixelate: f32,
}

impl RetroEffect {
    /// Virtuelle Zeilenzahl des groben Rasters.
    const LINES: f32 = 400.0;

    pub fn from_mode(mode: RetroMode) -> Self {
        match mode {
            RetroMode::Full => Self { virtual_lines: Self::LINES, dither: 1.0, palette: 1.0, pixelate: 1.0 },
            RetroMode::Palette => Self { virtual_lines: 100_000.0, dither: 1.0, palette: 1.0, pixelate: 0.0 },
            RetroMode::Off => Self { virtual_lines: 100_000.0, dither: 0.0, palette: 0.0, pixelate: 0.0 },
        }
    }
}

impl FullscreenMaterial for RetroEffect {
    fn fragment_shader() -> ShaderRef {
        "shaders/retro.wesl".into()
    }
}
