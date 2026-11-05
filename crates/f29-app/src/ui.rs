//! Bildschirme außerhalb des Cockpits (Menüs M-036, Nachbesprechung M-032)
//! mit bevy_ui in Departure Mono. Die UI zeichnet in ein Bild mit 640×400
//! Pixeln, das ohne Glättung in ganzen Vielfachen aufs Fenster vergrößert
//! wird – so bleiben die Schriftpixel scharf und gleich groß.
//!
//! Aufbau überall gleich: Titel, Textfeld fester Größe, darunter die Auswahl
//! (höchstens vier Zeilen, gewählte invers) immer an derselben Stelle. Auf
//! dem Bewaffnungsbildschirm liegen Umriss und Schächte (`hud.rs`) und die
//! Bilder der Ausrüstung (`store_art.rs`) in Fensterauflösung im Bildfeld
//! [`picture`] des Textfelds, auf der Lagebesprechung ebenso die Linien der
//! Karte ([`map_rect`]) über ihrem Geländebild. Das Titelbild ist eine
//! Rastergrafik (`assets/title.png`, 16:10, z. B. 320×200 oder 640×400),
//! bildfüllend.

use crate::HUD_LAYER;
use crate::menu::{self, Screen};
use crate::mfd::MapAssets;
use crate::roster::Duty;
use crate::sim::{SimState, SimSystems};
use crate::view::ViewSettings;
use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::text::{FontSmoothing, LineHeight};
use bevy::window::PrimaryWindow;
use f29_core::roster::SLOTS;
use f29_hud::arming::{Arming, ROWS, bay_rect};
use f29_hud::intel::{self, Label, MapView};
use f29_hud::map::map_half_extent;
use f29_hud::{Align, Ink, MAX_OPTIONS, Page, Rect, debrief};
use f29_render::palette;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<IntelView>()
            .add_systems(Startup, setup)
            .add_systems(Update, intel_input.in_set(SimSystems::Input))
            .add_systems(Update, (intel_map, show).in_set(SimSystems::Present));
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
/// Lagebesprechung: Kantenlänge der Karte rechts im Textfeld.
const MAP_PX: f32 = BODY_H - 2.0 * INSET;
/// Auswahl: Oberkante, Breite der Spalte, Breite und Abstand der Spalten
/// der Bewaffnung.
const OPTIONS_TOP: f32 = BODY_TOP + BODY_H + GAP;
const OPTIONS_W: f32 = 280.0;
const ARM_COL_W: f32 = 192.0;
const ARM_GAP: f32 = 8.0;

/// Was gerade zu sehen ist.
#[derive(Clone, PartialEq)]
enum Content {
    Title,
    Page(Page),
    Arming(Arming),
    Intel(Page, Vec<Label>),
}

#[derive(Resource)]
struct Ui {
    camera: Entity,
    font: Handle<Font>,
    title: Handle<Image>,
    /// Geländebild der Lagebesprechung und der Ausschnitt, den es zeigt.
    intel_map: Handle<Image>,
    intel_shown: Option<MapView>,
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

/// Innenfläche des Textfelds ab `left` Pixeln vom Bildrand (Einheiten des
/// 16:10-Bildschirms).
fn body_from(left: f32) -> Rect {
    let units = |x: f32, y: f32| [(x - W * 0.5) / H, (H * 0.5 - y) / H];
    let (top, right, bottom) = (BODY_TOP + INSET, W - PAD - INSET, BODY_TOP + BODY_H - INSET);
    Rect { min: units(left, bottom), max: units(right, top) }
}

/// Bildfeld der Bewaffnung: das Textfeld rechts der Textspalte.
pub fn picture() -> Rect {
    body_from(PAD + INSET + TEXT_COLUMN)
}

/// Karte der Lagebesprechung: quadratisch rechts im Textfeld.
pub fn map_rect() -> Rect {
    body_from(W - PAD - INSET - MAP_PX)
}

/// Punkt des 16:10-Bildschirms → Bildpixel.
fn to_px([x, y]: [f32; 2]) -> [f32; 2] {
    [W * 0.5 + x * H, H * 0.5 - y * H]
}

/// Bildpixel → Punkt des 16:10-Bildschirms.
fn to_units(p: Vec2) -> [f32; 2] {
    [(p.x - W * 0.5) / H, (H * 0.5 - p.y) / H]
}

/// Mauszeiger in Bildpixeln: Fensterpixel ohne Rand, durch die
/// Vergrößerung; außerhalb des Bildes `None`.
pub fn cursor_px(window: &Window) -> Option<Vec2> {
    let c = window.cursor_position()?;
    let p = (c - Vec2::new(window.width(), window.height()) * 0.5) / scale(window) + Vec2::new(W, H) * 0.5;
    (p.x >= 0.0 && p.y >= 0.0 && p.x < W && p.y < H).then_some(p)
}

/// Was unter dem Mauszeiger liegt: eine Zeile der Auswahl, ein Schacht
/// oder ein Platz des Dienstplans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Option(usize),
    Bay(usize),
    Row(usize),
}

/// Zeile `0..n` einer Auswahlspalte ab `x0` mit Breite `w` unter `p`.
fn slot(p: Vec2, x0: f32, w: f32, n: usize) -> Option<usize> {
    let (dx, dy) = (p.x - x0, p.y - OPTIONS_TOP);
    let k = (dy / OPTION_H) as usize;
    (dx >= 0.0 && dx < w && dy >= 0.0 && dy % OPTION_H < OPTION_H - 2.0 && k < n).then_some(k)
}

/// Treffer an `p` (Bildpixel) auf dem offenen Bildschirm – aus denselben
/// Maßen, mit denen die Bildschirme aufgebaut sind.
pub fn hit(state: &SimState, duty: &Duty, p: Vec2) -> Option<Hit> {
    match state.menu {
        // Plätze ab der zweiten Zeile des Textfelds (darüber die Kopfzeile)
        Some(Screen::Roster) => {
            let k = ((p.y - BODY_TOP - INSET) / LINE).floor() - 1.0;
            let row = (PAD + INSET..W - PAD - INSET).contains(&p.x) && (0.0..SLOTS as f32).contains(&k);
            let options = slot(p, (W - OPTIONS_W) * 0.5, OPTIONS_W, duty.page().options.len());
            options.map(Hit::Option).or(row.then_some(Hit::Row(k as usize)))
        }
        Some(Screen::Arming) => {
            let table = |k: usize| {
                let top = BODY_TOP + INSET + (3 + k) as f32 * LINE;
                (PAD + INSET..PAD + INSET + TEXT_COLUMN - 8.0).contains(&p.x)
                    && (top..top + LINE).contains(&p.y)
            };
            if let Some(k) = (0..4).find(|&k| table(k) || bay_rect(k, picture()).contains(to_units(p))) {
                return Some(Hit::Bay(k));
            }
            let n = menu::arming(state).items.len();
            let cols = n.div_ceil(ROWS);
            let x0 = (W - cols as f32 * (ARM_COL_W + ARM_GAP) + ARM_GAP) * 0.5;
            (0..cols).find_map(|c| {
                let k = slot(p, x0 + c as f32 * (ARM_COL_W + ARM_GAP), ARM_COL_W, ROWS)?;
                (c * ROWS + k < n).then_some(Hit::Option(c * ROWS + k))
            })
        }
        Some(Screen::Title) => None,
        // Menüs; ohne Menü die Nachbesprechung mit „CONTINUE“
        Some(_) => {
            slot(p, (W - OPTIONS_W) * 0.5, OPTIONS_W, menu::page(state).options.len()).map(Hit::Option)
        }
        None => slot(p, (W - OPTIONS_W) * 0.5, OPTIONS_W, 1).map(Hit::Option),
    }
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
    let size = Extent3d { width: MAP_PX as u32, height: MAP_PX as u32, depth_or_array_layers: 1 };
    let usage = RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD;
    let mut map =
        Image::new_fill(size, TextureDimension::D2, &[0, 0, 0, 255], TextureFormat::Rgba8UnormSrgb, usage);
    map.sampler = ImageSampler::nearest();
    let intel_map = images.add(map);
    commands.insert_resource(Ui {
        camera,
        font,
        title,
        intel_map,
        intel_shown: None,
        shown: None,
        root: None,
    });
}

/// Geländebild der Lagebesprechung: einmal je Mission aus dem Höhenbild
/// der Karte, sobald es berechnet ist.
fn intel_map(
    maps: Res<MapAssets>,
    state: Res<SimState>,
    intel: Res<IntelView>,
    mut ui: ResMut<Ui>,
    mut images: ResMut<Assets<Image>>,
) {
    if state.menu != Some(Screen::Intel) || ui.intel_shown == intel.view {
        return;
    }
    let (Some(view), Some(raster)) = (intel.view, maps.height_raster()) else { return };
    if let Some(mut image) = images.get_mut(&ui.intel_map) {
        image.data = Some(raster.render_view(view.center, 0.0, view.half, MAP_PX as usize));
        ui.intel_shown = intel.view;
    }
}

/// Ausschnitt der Lagebesprechungskarte; bei jeder neuen Mission wieder die
/// Route, beim Wechsel der Themen bleibt er.
#[derive(Resource, Default)]
pub struct IntelView {
    mission: Option<(u64, f29_core::rules::Conflict)>,
    pub view: Option<MapView>,
}

/// Mausrad zoomt zum Zeiger hin, mit gedrückter linker Taste verschiebt die
/// Maus die Karte – beides nur über der Karte. Verschoben wird um den Weg
/// des Zeigers, damit die Karte genau mitgeht.
fn intel_input(
    state: Res<SimState>,
    maps: Res<MapAssets>,
    window: Single<&Window, With<PrimaryWindow>>,
    scroll: Res<AccumulatedMouseScroll>,
    mut last: Local<Option<Vec2>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut intel: ResMut<IntelView>,
) {
    let Some(map) = maps.map.as_deref() else { return };
    let bound = map_half_extent(state.sim.terrain());
    if intel.mission != maps.mission() {
        intel.mission = maps.mission();
        intel.view = Some(MapView::fit(map, bound));
    }
    let cursor = cursor_px(&window);
    let moved = cursor.zip(*last).map(|(c, l)| c - l);
    *last = cursor;
    let (Some(mut view), Some(cursor)) = (intel.view, cursor) else { return };
    let (area, at) = (map_rect(), to_units(cursor));
    if state.menu != Some(Screen::Intel) || !area.contains(at) {
        return;
    }
    let lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    };
    if lines != 0.0 {
        view = view.zoom(area, at, 0.85_f64.powf(f64::from(lines)), bound);
    }
    if let Some(d) = moved.filter(|d| buttons.pressed(MouseButton::Left) && *d != Vec2::ZERO) {
        view = view.drag(area, [d.x / H, -d.y / H], bound);
    }
    if intel.view != Some(view) {
        intel.view = Some(view);
    }
}

/// Inhalt des offenen Bildschirms.
fn content(
    state: &SimState,
    duty: &Duty,
    view: &ViewSettings,
    maps: &MapAssets,
    intel: &IntelView,
) -> Option<Content> {
    match state.menu {
        Some(Screen::Title) => Some(Content::Title),
        Some(Screen::Roster) => Some(Content::Page(duty.page())),
        Some(Screen::Intel) => {
            let labels = maps.map.as_deref().zip(intel.view);
            let labels = labels.map(|(m, v)| intel::labels(m, menu::topic(state), v, map_rect()));
            Some(Content::Intel(menu::page(state), labels.unwrap_or_default()))
        }
        Some(Screen::Arming) => Some(Content::Arming(menu::arming(state))),
        Some(_) => Some(Content::Page(menu::page(state))),
        None if state.fd.mission_over => Some(Content::Page(debrief::page(&state.fd, view.debrief))),
        None => None,
    }
}

/// Baut die UI neu, sobald sich der Inhalt ändert.
#[allow(clippy::too_many_arguments)]
fn show(
    mut commands: Commands,
    window: Single<&Window, With<PrimaryWindow>>,
    state: Res<SimState>,
    duty: Res<Duty>,
    view: Res<ViewSettings>,
    maps: Res<MapAssets>,
    intel: Res<IntelView>,
    mut ui: ResMut<Ui>,
    mut cameras: Query<&mut Camera>,
    mut frame: Single<(&mut Sprite, &mut Visibility), With<Frame>>,
) {
    let content = content(&state, &duty, &view, &maps, &intel);
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
            Content::Intel(page, labels) => {
                let map = ui.intel_map.clone();
                root.with_children(|p| intel_ui(p, font, page, labels, map));
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
            let k = if r.big { 2.0 } else { 1.0 };
            row(b, font, (&r.text, &r.value), k, r.ink, r.inverse.then_some(Ink::Hud));
        }
    });
    options(p, font, page);
}

/// Die Auswahl der Seite, gewählte Zeile invers, markierte gedämpft invers.
fn options(p: &mut ChildSpawnerCommands, font: &Handle<Font>, page: &Page) {
    p.spawn(column(OPTIONS_W)).with_children(|list| {
        for (k, o) in page.options.iter().enumerate() {
            let bg = if k == page.cursor {
                Some(Ink::Hud)
            } else {
                (page.marked == Some(k)).then_some(Ink::HudDim)
            };
            option(list, font, o, bg);
        }
    });
}

/// Lagebesprechung: links der Text zum Thema, rechts die Karte (Geländebild,
/// darüber Linien aus `hud.rs` und die Beschriftung).
fn intel_ui(
    p: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    page: &Page,
    labels: &[Label],
    map: Handle<Image>,
) {
    title(p, font, &page.title);
    p.spawn(body(FlexDirection::Row)).with_children(|b| {
        b.spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, ..default() }).with_children(
            |c| {
                for r in &page.rows {
                    row(c, font, (&r.text, &r.value), 1.0, r.ink, None);
                }
            },
        );
        b.spawn((Node { width: px(MAP_PX), height: px(MAP_PX), ..default() }, ImageNode::new(map)));
    });
    options(p, font, page);
    for l in labels {
        let [x, y] = to_px(l.pos);
        let (left, right) = match l.align {
            Align::Right => (Val::Auto, px(W - x)),
            _ => (px(x), Val::Auto),
        };
        let top = px(y - LINE + 2.0);
        let node = Node { position_type: PositionType::Absolute, left, right, top, ..default() };
        p.spawn((node, text(font, &l.text, 1.0, color(l.ink))));
    }
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
    let cols = Node {
        width: percent(100),
        justify_content: JustifyContent::Center,
        column_gap: px(ARM_GAP),
        ..default()
    };
    p.spawn(cols).with_children(|cols| {
        for (c, items) in a.items.chunks(ROWS).enumerate() {
            cols.spawn(column(ARM_COL_W)).with_children(|list| {
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
