//! Grafiken der Ausrüstung für den Bewaffnungsbildschirm (M-036): je
//! Ausrüstung eine SVG-Datei `assets/stores/<code>.svg` (Code wie bei
//! `--loadout`, Seitenverhältnis etwa 2:1, Text als Pfade), beim Start mit
//! resvg gerastert und als Bild in den Schacht gelegt. Fehlt eine Datei,
//! zeigt der Schacht den Kurznamen.

use crate::HUD_LAYER;
use crate::sim::{SimState, SimSystems};
use bevy::asset::RenderAssetUsages;
use bevy::asset::io::file::FileAssetReader;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::PrimaryWindow;
use f29_core::weapons::WeaponKind;
use resvg::{tiny_skia, usvg};
use std::collections::HashMap;

pub struct StoreArtPlugin;

impl Plugin for StoreArtPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load).add_systems(Update, show.in_set(SimSystems::Present));
    }
}

/// Gerastertes Bild und Seitenverhältnis (Breite/Höhe) je Ausrüstung.
#[derive(Resource, Default)]
pub struct StoreArt(HashMap<WeaponKind, (Handle<Image>, f32)>);

/// Bild im Schacht `k`.
#[derive(Component)]
struct BayArt(usize);

/// Breite der Rastergrafik (Pixel).
const ART_PX: u32 = 512;

fn load(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let dir = FileAssetReader::get_base_path().join("assets/stores");
    let mut art = StoreArt::default();
    for kind in WeaponKind::ALL {
        let path = dir.join(format!("{}.svg", kind.code()));
        let Ok(data) = std::fs::read(&path) else { continue };
        match rasterize(&data) {
            Ok((image, aspect)) => {
                art.0.insert(kind, (images.add(image), aspect));
            }
            Err(e) => warn!("{}: {e}", path.display()),
        }
    }
    commands.insert_resource(art);
    for k in 0..4 {
        commands.spawn((
            Sprite::default(),
            Transform::from_xyz(0.0, 0.0, -4.0),
            Visibility::Hidden,
            RenderLayers::layer(HUD_LAYER),
            BayArt(k),
        ));
    }
}

/// SVG → Bild mit [`ART_PX`] Breite, nicht vormultipliziert (wie Bevy es
/// erwartet).
fn rasterize(data: &[u8]) -> Result<(Image, f32), String> {
    let tree = usvg::Tree::from_data(data, &usvg::Options::default()).map_err(|e| e.to_string())?;
    let size = tree.size();
    let aspect = size.width() / size.height();
    let (w, h) = (ART_PX, ((ART_PX as f32 / aspect).round() as u32).max(1));
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or("Größe ungültig")?;
    let scale = w as f32 / size.width();
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let rgba = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    let extent = Extent3d { width: w, height: h, depth_or_array_layers: 1 };
    let usage = RenderAssetUsages::RENDER_WORLD;
    Ok((Image::new(extent, TextureDimension::D2, rgba, TextureFormat::Rgba8UnormSrgb, usage), aspect))
}

/// Auf dem Bewaffnungsbildschirm: je Schacht das Bild seiner Ausrüstung,
/// eingepasst in das Bildfeld des Schachts.
fn show(
    window: Single<&Window, With<PrimaryWindow>>,
    state: Res<SimState>,
    art: Res<StoreArt>,
    mut sprites: Query<(&BayArt, &mut Sprite, &mut Transform, &mut Visibility)>,
) {
    let h = crate::ui::unit(&window);
    let arming = state.menu == Some(crate::menu::Screen::Arming);
    let bays = state.sim.stores().bays;
    for (bay, mut sprite, mut t, mut vis) in &mut sprites {
        let image = bays[bay.0].kind.filter(|_| arming).and_then(|k| art.0.get(&k));
        let Some((handle, aspect)) = image else {
            *vis = Visibility::Hidden;
            continue;
        };
        let r = f29_hud::arming::art_rect(bay.0, crate::ui::picture());
        let (w, rh) = (r.max[0] - r.min[0], r.max[1] - r.min[1]);
        let size = if w / rh > *aspect { Vec2::new(rh * aspect, rh) } else { Vec2::new(w, w / aspect) };
        sprite.image = handle.clone();
        sprite.custom_size = Some(size * h);
        let c = r.center();
        t.translation = Vec3::new(c[0] * h, c[1] * h, -4.0);
        *vis = Visibility::Inherited;
    }
}
