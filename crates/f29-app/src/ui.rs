//! Bildschirme außerhalb des Cockpits (Menüs M-036, Nachbesprechung M-032)
//! mit bevy_ui in Departure Mono. Die UI zeichnet in ein Bild mit 640×400
//! Pixeln, das ohne Glättung in ganzen Vielfachen aufs Fenster vergrößert
//! wird – so bleiben die Schriftpixel scharf und gleich groß.
//!
//! Aufbau überall gleich: Titel, Textfeld fester Größe, darunter die Auswahl
//! (höchstens vier Zeilen, gewählte invers) immer an derselben Stelle. Auf
//! dem Bewaffnungsbildschirm liegen Umriss und Schächte (`hud.rs`) und die
//! Bilder der Ausrüstung (`store_art.rs`) in Fensterauflösung im Bildfeld
//! [`picture`] des Textfelds. Das Titelbild ist eine Rastergrafik
//! (`assets/title.png`, 16:10, z. B. 320×200 oder 640×400), bildfüllend.

use crate::HUD_LAYER;
use crate::menu::{self, Screen};
use crate::sim::{SimState, SimSystems};
use crate::view::ViewSettings;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::text::{FontSmoothing, LineHeight};
use bevy::window::PrimaryWindow;
use f29_hud::arming::{Arming, ROWS};
use f29_hud::{Ink, MAX_OPTIONS, Page, Rect, debrief};
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
/// Rand und Abstand der Blöcke; Höhe von Titel, Zeile der Auswahl und
/// Auswahl; Rahmen samt Innenabstand des Textfelds (Pixel).
const PAD: f32 = 16.0;
const GAP: f32 = 12.0;
const TITLE_H: f32 = 2.0 * LINE + 8.0;
const OPTION_H: f32 = LINE + 6.0;
const OPTIONS_H: f32 = MAX_OPTIONS as f32 * OPTION_H;
const INSET: f32 = 9.0;
/// Textfeld: oberer Rand und Höhe.
const BODY_TOP: f32 = PAD + TITLE_H + GAP;
const BODY_H: f32 = H - 2.0 * PAD - TITLE_H - 2.0 * GAP - OPTIONS_H;
/// Bewaffnung: Breite der Textspalte links im Textfeld.
const TEXT_COLUMN: f32 = 170.0;

/// Was gerade zu sehen ist.
#[derive(Clone, PartialEq)]
enum Content {
    Title,
    Page(Page),
    Arming(Arming),
}

#[derive(Resource)]
struct Ui {
    camera: Entity,
    font: Handle<Font>,
    title: Handle<Image>,
    shown: Option<Content>,
    root: Option<Entity>,
}

/// Das Bild der UI im Overlay.
#[derive(Component)]
struct Frame;

/// Fensterpixel (logisch) je Bildpixel: ganze Vielfache, solange das
/// Fenster groß genug ist.
fn scale(window: &Window) -> f32 {
    let s = (window.physical_width() as f32 / W).min(window.physical_height() as f32 / H);
    (if s >= 1.0 { s.floor() } else { s }) / window.scale_factor()
}

/// Fensterpixel (logisch) je Einheit des 16:10-Bildschirms (Bildhöhe = 1,
/// Mitte 0,0, y nach oben), für Linien und Bilder über der UI.
pub fn unit(window: &Window) -> f32 {
    scale(window) * H
}

/// Bildfeld der Bewaffnung (Einheiten des 16:10-Bildschirms): das Textfeld
/// rechts der Textspalte.
pub fn picture() -> Rect {
    let units = |x: f32, y: f32| [(x - W * 0.5) / H, (H * 0.5 - y) / H];
    let (left, top) = (PAD + INSET + TEXT_COLUMN, BODY_TOP + INSET);
    let (right, bottom) = (W - PAD - INSET, BODY_TOP + BODY_H - INSET);
    Rect { min: units(left, bottom), max: units(right, top) }
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
        Frame,
    ));
    let font = assets.load("fonts/departure-mono.ttf");
    let nearest = |s: &mut ImageLoaderSettings| s.sampler = ImageSampler::nearest();
    let title = assets.load_builder().with_settings(nearest).load("title.png");
    commands.insert_resource(Ui { camera, font, title, shown: None, root: None });
}

/// Inhalt des offenen Bildschirms.
fn content(state: &SimState, view: &ViewSettings) -> Option<Content> {
    match state.menu {
        Some(Screen::Title) => Some(Content::Title),
        Some(Screen::Arming) => Some(Content::Arming(menu::arming(state))),
        Some(_) => Some(Content::Page(menu::page(state))),
        None if state.fd.mission_over => Some(Content::Page(debrief::page(&state.fd, view.debrief))),
        None => None,
    }
}

/// Baut die UI neu, sobald sich der Inhalt ändert.
fn show(
    mut commands: Commands,
    window: Single<&Window, With<PrimaryWindow>>,
    state: Res<SimState>,
    view: Res<ViewSettings>,
    mut ui: ResMut<Ui>,
    mut cameras: Query<&mut Camera>,
    mut frame: Single<(&mut Sprite, &mut Visibility), With<Frame>>,
) {
    let content = content(&state, &view);
    let on = content.is_some();
    *frame.1 = if on { Visibility::Visible } else { Visibility::Hidden };
    frame.0.custom_size = Some(Vec2::new(W, H) * scale(&window));
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
        let font = &ui.font;
        let mut root = commands.spawn((root_node(), UiTargetCamera(ui.camera)));
        match c {
            Content::Title => {
                root.insert((
                    Node { padding: UiRect::ZERO, ..root_node() },
                    ImageNode::new(ui.title.clone()),
                ));
            }
            Content::Page(page) => {
                root.with_children(|p| page_ui(p, font, page));
            }
            Content::Arming(a) => {
                root.with_children(|p| arming_ui(p, font, a));
            }
        }
        ui.root = Some(root.id());
    }
    ui.shown = content;
}

/// Wurzel: das ganze Bild, Blöcke untereinander.
fn root_node() -> Node {
    Node {
        width: percent(100),
        height: percent(100),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        padding: UiRect::all(px(PAD)),
        row_gap: px(GAP),
        ..default()
    }
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
        height: px(TITLE_H),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    };
    p.spawn((node, BackgroundColor(color(Ink::Hud)))).with_child(text(font, s, 2.0, Color::BLACK));
}

/// Textfeld: so breit wie der Titel, auf jedem Bildschirm gleich hoch.
fn body(direction: FlexDirection) -> impl Bundle {
    let node = Node {
        width: percent(100),
        height: px(BODY_H),
        flex_direction: direction,
        padding: UiRect::all(px(INSET - 1.0)),
        border: UiRect::all(px(1)),
        overflow: Overflow::clip(),
        ..default()
    };
    (node, BorderColor::all(color(Ink::HudDim)))
}

/// Zeile im Textfeld: Text links, Wert rechts; `bg` hinterlegt sie invers.
fn row(
    p: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    (text_s, value): (&str, &str),
    k: f32,
    ink: Ink,
    bg: Option<Ink>,
) {
    let justify = if k > 1.0 { JustifyContent::Center } else { JustifyContent::SpaceBetween };
    let node = Node {
        height: px(LINE * k),
        justify_content: justify,
        padding: UiRect::horizontal(px(2)),
        ..default()
    };
    let (bg, fg) = match bg {
        Some(b) => (color(b), Color::BLACK),
        None => (Color::NONE, color(ink)),
    };
    p.spawn((node, BackgroundColor(bg))).with_children(|r| {
        r.spawn(text(font, text_s, k, fg));
        r.spawn(text(font, value, k, fg));
    });
}

/// Zeile der Auswahl: `bg` hinterlegt sie invers (gewählt), sonst gedämpft.
fn option(p: &mut ChildSpawnerCommands, font: &Handle<Font>, s: &str, bg: Option<Ink>) {
    let node = Node {
        height: px(OPTION_H - 2.0),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        margin: UiRect::bottom(px(2)),
        ..default()
    };
    let (bg, fg) = match bg {
        Some(b) => (color(b), Color::BLACK),
        None => (Color::NONE, color(Ink::HudDim)),
    };
    p.spawn((node, BackgroundColor(bg))).with_child(text(font, s, 1.0, fg));
}

/// Spalte der Auswahl, `width` Pixel breit.
fn column(width: f32) -> Node {
    Node { width: px(width), height: px(OPTIONS_H), flex_direction: FlexDirection::Column, ..default() }
}

fn page_ui(p: &mut ChildSpawnerCommands, font: &Handle<Font>, page: &Page) {
    title(p, font, &page.title);
    p.spawn(body(FlexDirection::Column)).with_children(|b| {
        for r in &page.rows {
            row(b, font, (&r.text, &r.value), if r.big { 2.0 } else { 1.0 }, r.ink, None);
        }
    });
    p.spawn(column(280.0)).with_children(|list| {
        for (k, o) in page.options.iter().enumerate() {
            option(list, font, o, (k == page.cursor).then_some(Ink::Hud));
        }
    });
}

/// Bewaffnung: links im Textfeld Treibstoff und Schächte (der im Fokus
/// invers), rechts das Bildfeld; darunter die Liste in Spalten zu je
/// [`ROWS`]. Der Eintrag am Cursor ist invers, gedämpft, solange ein Schacht
/// den Fokus hat.
fn arming_ui(p: &mut ChildSpawnerCommands, font: &Handle<Font>, a: &Arming) {
    title(p, font, "ARMING");
    let (need, have) = a.fuel;
    let short = if have < need { Ink::HudWarning } else { Ink::Hud };
    p.spawn(body(FlexDirection::Row)).with_children(|b| {
        let col = Node { width: px(TEXT_COLUMN - 8.0), flex_direction: FlexDirection::Column, ..default() };
        b.spawn(col).with_children(|c| {
            row(c, font, ("FUEL REQUIRED", &format!("{need:.0} KG")), 1.0, Ink::Hud, None);
            row(c, font, ("FUEL CARRIED", &format!("{have:.0} KG")), 1.0, short, None);
            row(c, font, ("", ""), 1.0, Ink::Hud, None);
            for k in 0..4 {
                let (name, count) = match &a.bays[k] {
                    Some((n, c)) => (n.as_str(), format!("X{c}")),
                    None => ("EMPTY", String::new()),
                };
                let label = format!("BAY {} {name}", k + 1);
                row(c, font, (&label, &count), 1.0, Ink::Hud, (a.bay == Some(k)).then_some(Ink::Hud));
            }
        });
    });
    let cols =
        Node { width: percent(100), justify_content: JustifyContent::Center, column_gap: px(8), ..default() };
    p.spawn(cols).with_children(|cols| {
        for (c, items) in a.items.chunks(ROWS).enumerate() {
            cols.spawn(column(192.0)).with_children(|list| {
                for (i, item) in items.iter().enumerate() {
                    let bg = (c * ROWS + i == a.cursor).then_some(if a.bay.is_some() {
                        Ink::HudDim
                    } else {
                        Ink::Hud
                    });
                    option(list, font, item, bg);
                }
            });
        }
    });
}
