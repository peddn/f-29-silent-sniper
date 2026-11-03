//! Prototyp: die Schlussauswahl (Final Choices) mit bevy_ui statt der
//! Vektorschrift, in drei Pixelschriften zum Vergleich (Taste X wechselt).
//! Die UI zeichnet in ein Bild mit 640×400 Pixeln, das ohne Glättung in
//! ganzen Vielfachen aufs Fenster vergrößert wird – so bleiben die Pixel
//! der Schriften scharf und gleich groß.

use crate::HUD_LAYER;
use crate::menu::Screen;
use crate::sim::{SimState, SimSystems};
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::text::{FontSmoothing, LineHeight};
use bevy::window::PrimaryWindow;
use f29_hud::Ink;
use f29_render::palette;

pub struct UiProtoPlugin;

impl Plugin for UiProtoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, update.in_set(SimSystems::Present));
    }
}

/// Größe des Bildes (Pixel).
const W: u32 = 640;
const H: u32 = 400;

/// Name, Datei, Größe, bei der ein Schriftpixel ein Bildpixel ist, und
/// Zeilenhöhe in ganzen Pixeln (sonst stehen die Zeilen ungleich weit auseinander).
const FONTS: [(&str, &str, f32, f32); 3] = [
    ("GOHUFONT 14", "fonts/gohufont-14.ttf", 14.0, 16.0),
    ("PROGGY CLEAN", "fonts/proggy-clean.ttf", 16.0, 15.0),
    ("DEPARTURE MONO", "fonts/departure-mono.ttf", 11.0, 14.0),
];

const OPTIONS: [&str; 4] = ["SELECT NEW MISSION", "INTELLIGENCE BRIEFING", "ARM YOUR PLANE", "BEGIN MISSION"];

#[derive(Resource)]
struct Proto {
    camera: Entity,
    fonts: [Handle<Font>; 3],
    font: usize,
}

/// Bild der UI im Overlay.
#[derive(Component)]
struct Screenshot;

/// Text in der gewählten Schrift; das Feld ist das Vielfache ihrer Grundgröße.
#[derive(Component)]
struct Styled(f32);

#[derive(Component)]
struct Choice(usize);

#[derive(Component)]
struct Summary;

#[derive(Component)]
struct Footer;

fn color(ink: Ink) -> Color {
    let [r, g, b] = ink.srgb();
    Color::srgb(r, g, b)
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, assets: Res<AssetServer>) {
    let mut image = Image::new_target_texture(W, H, TextureFormat::Rgba8UnormSrgb, None);
    image.sampler = ImageSampler::nearest();
    let image = images.add(image);
    let [r, g, b] = palette::mix(0, 13, 0.08);
    let camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: -2,
                is_active: false,
                clear_color: ClearColorConfig::Custom(Color::srgb(r, g, b)),
                ..default()
            },
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
        Screenshot,
    ));
    commands.insert_resource(Proto { camera, fonts: FONTS.map(|f| assets.load(f.1)), font: 0 });

    let (hud, dim) = (color(Ink::Hud), color(Ink::HudDim));
    let text = |s: &str, size: f32, c: Color| (Text::new(s), TextColor(c), Styled(size));
    let full = Node { width: percent(100), height: percent(100), ..default() };
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(px(16)),
                row_gap: px(12),
                ..full
            },
            UiTargetCamera(camera),
        ))
        .with_children(|root| {
            // Titel invers über die ganze Breite
            root.spawn((
                Node {
                    width: percent(100),
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(px(0), px(4)),
                    ..default()
                },
                BackgroundColor(hud),
            ))
            .with_child(text("FINAL CHOICES", 2.0, Color::BLACK));
            // Auftrag in Kürze, gerahmt
            root.spawn((
                Node {
                    width: px(480),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(8)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BorderColor::all(dim),
            ))
            .with_child((text("", 1.0, hud), Summary));
            // Auswahl: gewählte Zeile invers
            root.spawn(Node {
                width: px(280),
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                ..default()
            })
            .with_children(|list| {
                for (k, option) in OPTIONS.iter().enumerate() {
                    list.spawn((
                        Node {
                            justify_content: JustifyContent::Center,
                            padding: UiRect::axes(px(0), px(2)),
                            ..default()
                        },
                        Choice(k),
                    ))
                    .with_child(text(option, 1.0, dim));
                }
            });
            root.spawn(Node { flex_grow: 1.0, ..default() });
            root.spawn((text("", 1.0, dim), Footer));
        });
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn update(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<SimState>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut proto: ResMut<Proto>,
    mut cameras: Query<&mut Camera>,
    mut shot: Single<(&mut Sprite, &mut Visibility), With<Screenshot>>,
    mut styled: Query<(&mut TextFont, &mut LineHeight, &Styled)>,
    mut choices: Query<(&Choice, &mut BackgroundColor, &Children)>,
    mut texts: ParamSet<(
        Query<&mut TextColor>,
        Single<&mut Text, With<Summary>>,
        Single<&mut Text, With<Footer>>,
    )>,
) {
    let open = state.menu == Some(Screen::Final);
    *shot.1 = if open { Visibility::Visible } else { Visibility::Hidden };
    if let Ok(mut camera) = cameras.get_mut(proto.camera) {
        camera.is_active = open;
    }
    if !open {
        return;
    }
    if keys.just_pressed(KeyCode::KeyX) {
        proto.font = (proto.font + 1) % FONTS.len();
    }
    // ganze Vielfache der Bildpixel, solange das Fenster groß genug ist
    let s = (window.physical_width() as f32 / W as f32).min(window.physical_height() as f32 / H as f32);
    let s = if s >= 1.0 { s.floor() } else { s } / window.scale_factor();
    shot.0.custom_size = Some(Vec2::new(W as f32, H as f32) * s);

    let (_, _, size, line) = FONTS[proto.font];
    for (mut font, mut height, styled) in &mut styled {
        height.set_if_neq(LineHeight::Px(line * styled.0));
        font.set_if_neq(TextFont {
            font: proto.fonts[proto.font].clone().into(),
            font_size: (size * styled.0).into(),
            font_smoothing: FontSmoothing::None,
            ..default()
        });
    }
    let (hud, dim) = (color(Ink::Hud), color(Ink::HudDim));
    for (choice, mut bg, children) in &mut choices {
        let on = choice.0 == state.cursor;
        bg.set_if_neq(BackgroundColor(if on { hud } else { Color::NONE }));
        if let Ok(mut c) = texts.p0().get_mut(children[0]) {
            c.set_if_neq(TextColor(if on { Color::BLACK } else { dim }));
        }
    }
    let summary = state.sim.briefing().join("\n");
    texts.p1().set_if_neq(Text(summary));
    let footer = format!("UP/DOWN SELECT   SPACE OK   X FONT: {}", FONTS[proto.font].0);
    texts.p2().set_if_neq(Text(footer));
}
