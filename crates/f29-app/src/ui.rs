//! Bildschirme außerhalb des Cockpits (Menüs M-036, Nachbesprechung M-032)
//! mit bevy_ui in Departure Mono. Die UI zeichnet in ein Bild mit 640×400
//! Pixeln, das ohne Glättung in ganzen Vielfachen aufs Fenster vergrößert
//! wird – so bleiben die Schriftpixel scharf und gleich groß.
//!
//! Aufbau überall gleich: Titel, Textfeld fester Größe, darunter die Auswahl
//! (höchstens vier Zeilen) immer an derselben Stelle. Auf dem
//! Bewaffnungsbildschirm liegen Umriss und Schächte (`hud.rs`) und die
//! Bilder der Ausrüstung (`store_art.rs`) in Fensterauflösung darüber, im
//! selben Rahmen ([`unit`]).

use crate::HUD_LAYER;
use crate::menu::{self, Screen};
use crate::sim::{SimState, SimSystems};
use crate::store_art::StoreArt;
use crate::view::ViewSettings;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::text::{FontSmoothing, LineHeight};
use bevy::window::PrimaryWindow;
use f29_hud::arming::{Arming, PICTURE, ROWS, bay_rect};
use f29_hud::{Ink, MAX_OPTIONS, Page, debrief};
use f29_render::palette;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, show.in_set(SimSystems::Present));
    }
}

/// Größe des Bildes (Pixel).
const W: f32 = 640.0;
const H: f32 = 400.0;
/// Schriftgröße, bei der ein Schriftpixel ein Bildpixel ist, und Zeilenhöhe
/// in ganzen Pixeln (sonst stehen die Zeilen ungleich weit auseinander).
const SIZE: f32 = 11.0;
const LINE: f32 = 14.0;
/// Rand und Abstand der Blöcke (Pixel).
const PAD: f32 = 16.0;
const GAP: f32 = 12.0;

/// Was gerade zu sehen ist.
#[derive(Clone, PartialEq)]
enum Content {
    Page(Page),
    Arming(Arming),
}

#[derive(Resource)]
struct Ui {
    camera: Entity,
    font: Handle<Font>,
    shown: Option<Content>,
    root: Option<Entity>,
}

/// Das Bild der UI im Overlay.
#[derive(Component)]
struct Picture;

/// Fensterpixel (logisch) je Bildpixel: ganze Vielfache, solange das
/// Fenster groß genug ist.
fn scale(window: &Window) -> f32 {
    let s = (window.physical_width() as f32 / W).min(window.physical_height() as f32 / H);
    (if s >= 1.0 { s.floor() } else { s }) / window.scale_factor()
}

/// Fensterpixel (logisch) je Einheit des 16:10-Bildschirms (Bildhöhe = 1),
/// für Linien und Bilder über der UI.
pub fn unit(window: &Window) -> f32 {
    scale(window) * H
}

fn color(ink: Ink) -> Color {
    let [r, g, b] = ink.srgb();
    Color::srgb(r, g, b)
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, assets: Res<AssetServer>) {
    let mut image = Image::new_target_texture(W as u32, H as u32, TextureFormat::Rgba8UnormSrgb, None);
    image.sampler = ImageSampler::nearest();
    let image = images.add(image);
    let [r, g, b] = palette::mix(0, 13, 0.08);
    let clear_color = ClearColorConfig::Custom(Color::srgb(r, g, b));
    let camera = commands
        .spawn((
            Camera2d,
            Camera { order: -2, is_active: false, clear_color, ..default() },
            RenderTarget::from(image.clone()),
            Tonemapping::None,
            Msaa::Off,
        ))
        .id();
    commands.spawn((
        Sprite::from_image(image),
        Transform::from_xyz(0.0, 0.0, -5.0),
        Visibility::Hidden,
        RenderLayers::layer(HUD_LAYER),
        Picture,
    ));
    let font = assets.load("fonts/departure-mono.ttf");
    commands.insert_resource(Ui { camera, font, shown: None, root: None });
}

/// Inhalt des offenen Bildschirms.
fn content(state: &SimState, view: &ViewSettings, art: &StoreArt, blink: bool) -> Option<Content> {
    match state.menu {
        Some(Screen::Arming) => Some(Content::Arming(menu::arming(state, art, blink))),
        Some(_) => Some(Content::Page(menu::page(state))),
        None if state.fd.mission_over => Some(Content::Page(debrief::page(&state.fd, view.debrief))),
        None => None,
    }
}

/// Baut die UI neu, sobald sich der Inhalt ändert.
#[allow(clippy::too_many_arguments)]
fn show(
    mut commands: Commands,
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
    state: Res<SimState>,
    view: Res<ViewSettings>,
    art: Res<StoreArt>,
    mut ui: ResMut<Ui>,
    mut cameras: Query<&mut Camera>,
    mut picture: Single<(&mut Sprite, &mut Visibility), With<Picture>>,
) {
    let content = content(&state, &view, &art, time.elapsed_secs().fract() < 0.5);
    let on = content.is_some();
    *picture.1 = if on { Visibility::Visible } else { Visibility::Hidden };
    picture.0.custom_size = Some(Vec2::new(W, H) * scale(&window));
    if let Ok(mut camera) = cameras.get_mut(ui.camera) {
        camera.is_active = on;
    }
    if content == ui.shown {
        return;
    }
    if let Some(root) = ui.root.take() {
        commands.entity(root).despawn();
    }
    if let Some(c) = &content {
        let node = Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::all(px(PAD)),
            row_gap: px(GAP),
            ..default()
        };
        let font = &ui.font;
        let mut root = commands.spawn((node, UiTargetCamera(ui.camera)));
        root.with_children(|p| match c {
            Content::Page(page) => page_ui(p, font, page),
            Content::Arming(a) => arming_ui(p, font, a),
        });
        ui.root = Some(root.id());
    }
    ui.shown = content;
}

/// Text in Departure Mono, `k`-fach groß.
fn text(font: &Handle<Font>, s: &str, k: f32, color: Color) -> impl Bundle {
    let font = TextFont {
        font: font.clone().into(),
        font_size: (SIZE * k).into(),
        font_smoothing: FontSmoothing::None,
        ..default()
    };
    (Text::new(s), font, LineHeight::Px(LINE * k), TextColor(color))
}

/// Titel invers über die ganze Breite.
fn title(p: &mut ChildSpawnerCommands, font: &Handle<Font>, s: &str) {
    let node = Node {
        width: percent(100),
        justify_content: JustifyContent::Center,
        padding: UiRect::axes(px(0), px(4)),
        ..default()
    };
    p.spawn((node, BackgroundColor(color(Ink::Hud)))).with_child(text(font, s, 2.0, Color::BLACK));
}

fn page_ui(p: &mut ChildSpawnerCommands, font: &Handle<Font>, page: &Page) {
    title(p, font, &page.title);
    // Textfeld: füllt den Platz bis zur Auswahl, auf jedem Bildschirm gleich
    let body = Node {
        width: px(480),
        flex_grow: 1.0,
        flex_direction: FlexDirection::Column,
        padding: UiRect::all(px(8)),
        border: UiRect::all(px(1)),
        overflow: Overflow::clip(),
        ..default()
    };
    p.spawn((body, BorderColor::all(color(Ink::HudDim)))).with_children(|body| {
        for row in &page.rows {
            let (k, justify) =
                if row.big { (2.0, JustifyContent::Center) } else { (1.0, JustifyContent::SpaceBetween) };
            let node = Node { height: px(LINE * k), justify_content: justify, ..default() };
            body.spawn(node).with_children(|r| {
                r.spawn(text(font, &row.text, k, color(row.ink)));
                r.spawn(text(font, &row.value, k, color(row.ink)));
            });
        }
    });
    // Auswahl mit fester Höhe, gewählte Zeile invers
    let height = MAX_OPTIONS as f32 * (LINE + 6.0);
    let list =
        Node { width: px(280), height: px(height), flex_direction: FlexDirection::Column, ..default() };
    p.spawn(list).with_children(|list| {
        for (k, option) in page.options.iter().enumerate() {
            let on = k == page.cursor;
            let node = Node {
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(0), px(2)),
                margin: UiRect::bottom(px(2)),
                ..default()
            };
            let (bg, fg) =
                if on { (color(Ink::Hud), Color::BLACK) } else { (Color::NONE, color(Ink::HudDim)) };
            list.spawn((node, BackgroundColor(bg))).with_child(text(font, option, 1.0, fg));
        }
    });
}

/// Punkt des 16:10-Bildschirms (Bildhöhe = 1, Mitte 0,0, y nach oben) →
/// Bildpixel.
fn to_px([x, y]: [f32; 2]) -> [f32; 2] {
    [W * 0.5 + x * H, H * 0.5 - y * H]
}

fn arming_ui(p: &mut ChildSpawnerCommands, font: &Handle<Font>, a: &Arming) {
    title(p, font, "ARMING");
    let (need, have) = a.fuel;
    let ink = if have < need { Ink::HudWarning } else { Ink::HudDim };
    p.spawn(text(font, &format!("FUEL REQUIRED {need:.0} KG    CARRIED {have:.0} KG"), 1.0, color(ink)));
    let blink = color(if a.blink { Ink::HudWarning } else { Ink::Hud });
    let dim = color(Ink::HudDim);
    // Beschriftung der Schächte über den Linien
    for k in 0..4 {
        let r = bay_rect(k);
        let ([left, top], [right, bottom]) = (to_px([r.min[0], r.max[1]]), to_px([r.max[0], r.min[1]]));
        let node = Node {
            position_type: PositionType::Absolute,
            left: px(left),
            top: px(top),
            width: px(right - left),
            height: px(bottom - top),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            padding: UiRect::all(px(3)),
            ..default()
        };
        let ink = if a.bay == Some(k) { blink } else { color(Ink::Hud) };
        let (name, count) = match &a.bays[k] {
            Some((n, c)) => (n.as_str(), format!("X{c}")),
            None => ("EMPTY", String::new()),
        };
        p.spawn(node).with_children(|b| {
            b.spawn(text(font, &format!("BAY {}", k + 1), 1.0, dim));
            b.spawn(text(font, if a.art[k] { "" } else { name }, 1.0, ink));
            b.spawn(text(font, &count, 1.0, ink));
        });
    }
    // Liste in zwei Spalten unter dem Bild; der Eintrag am Cursor blinkt,
    // solange kein Schacht den Fokus hat.
    let node = Node {
        position_type: PositionType::Absolute,
        top: px(to_px([0.0, PICTURE[0]])[1] + 8.0),
        width: percent(100),
        justify_content: JustifyContent::Center,
        column_gap: px(48),
        ..default()
    };
    p.spawn(node).with_children(|cols| {
        for (c, column) in a.items.chunks(ROWS).enumerate() {
            let node = Node { width: px(220), flex_direction: FlexDirection::Column, ..default() };
            cols.spawn(node).with_children(|col| {
                for (i, item) in column.iter().enumerate() {
                    let ink = match (c * ROWS + i == a.cursor, a.bay) {
                        (true, None) => blink,
                        (true, Some(_)) => color(Ink::Hud),
                        _ => dim,
                    };
                    col.spawn(text(font, item, 1.0, ink));
                }
            });
        }
    });
}
