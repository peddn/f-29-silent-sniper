//! Oberfläche des Karteneditors (egui): links Werkzeuge, in der Mitte die
//! Karte, oben Datei und Verlauf, unten die Lage unter der Maus.

use crate::edit::{self, History, Paint};
use crate::preview;
use crate::recent::{Recent, absolute};
use eframe::egui::{
    self, Align2, Color32, ColorImage, FontId, Key, Modifiers, PointerButton, Pos2, Rect, Sense,
    TextureHandle, TextureOptions, Vec2, ViewportCommand, pos2, vec2,
};
use f29_core::nav::grid_square;
use f29_render::palette;
use f29_world::theater::{CELL_M, GRID_N, Grid, SIZE_M, THEATER_DIR};
use f29_world::{AirfieldKind, Ground, Side, Theater};
use std::path::{Path, PathBuf};

/// Pinselgrößen (Zellen Durchmesser).
const SIZES: [usize; 5] = [1, 3, 5, 9, 15];
/// Zoom (Pixel je Zelle).
const ZOOM_MIN: f32 = 1.0;
const ZOOM_MAX: f32 = 64.0;

fn ground_name(g: Ground) -> &'static str {
    match g {
        Ground::Water => "Wasser",
        Ground::Plain => "Flachland",
        Ground::Hills => "Hügel",
        Ground::Mountains => "Gebirge",
        Ground::Desert => "Wüste",
        Ground::Forest => "Wald",
        Ground::City => "Stadt",
        Ground::Village => "Dorf",
    }
}

fn side_name(s: Side) -> &'static str {
    match s {
        Side::Friendly => "eigen",
        Side::Neutral => "neutral",
        Side::Hostile => "feindlich",
    }
}

/// Farbe einer Geländeart im Raster, aus der Palette des Spiels.
fn ground_color(g: Ground) -> Color32 {
    let [r, gr, b] = match g {
        Ground::Water => palette::srgb(2),
        Ground::Plain => palette::srgb(9),
        Ground::Hills => palette::srgb(11),
        Ground::Mountains => palette::srgb(14),
        Ground::Desert => palette::srgb(10),
        Ground::Forest => palette::srgb(7),
        Ground::City => palette::srgb(15),
        Ground::Village => palette::mix(10, 15, 0.5),
    }
    .map(|c| (c * 255.0).round() as u8);
    Color32::from_rgb(r, gr, b)
}

fn side_color(s: Side) -> Color32 {
    match s {
        Side::Friendly => Color32::from_rgb(60, 120, 235),
        Side::Neutral => Color32::from_rgb(235, 205, 60),
        Side::Hostile => Color32::from_rgb(225, 40, 40),
    }
}

/// Ein Startwert für neue Gebiete (einmal gesetzt, danach fest).
fn new_seed() -> u64 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    u64::from(t.subsec_nanos() ^ t.as_secs() as u32) % 100_000
}

/// Liest ein PNG als RGBA mit 8 Bit je Kanal.
fn read_png(path: &Path) -> Result<([usize; 2], Vec<u8>), String> {
    let err = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let file = std::fs::File::open(path).map_err(|e| err(&e))?;
    let mut dec = png::Decoder::new(std::io::BufReader::new(file));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().map_err(|e| err(&e))?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or_else(|| err(&"Bild zu groß"))?];
    let info = reader.next_frame(&mut buf).map_err(|e| err(&e))?;
    let (w, h) = (info.width as usize, info.height as usize);
    let channels = info.color_type.samples();
    let rgba = buf[..w * h * channels]
        .chunks_exact(channels)
        .flat_map(|p| match *p {
            [g] => [g, g, g, 255],
            [g, a] => [g, g, g, a],
            [r, g, b] => [r, g, b, 255],
            [r, g, b, a] => [r, g, b, a],
            _ => [0; 4],
        })
        .collect();
    Ok(([w, h], rgba))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer {
    Ground,
    Side,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
    Brush,
    Fill,
}

/// Wofür ein Pfad gefragt wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ask {
    Open,
    SaveAs,
    Underlay,
    Mask,
}

impl Ask {
    fn title(self) -> &'static str {
        match self {
            Ask::Open => "Gebiet öffnen",
            Ask::SaveAs => "Speichern unter",
            Ask::Underlay => "Unterlage laden (PNG über das ganze Gebiet, Norden oben)",
            Ask::Mask => "Wasser aus Maske (PNG, dunkel = Wasser, hell = Land)",
        }
    }

    fn ext(self) -> &'static str {
        if matches!(self, Ask::Open | Ask::SaveAs) { "ron" } else { "png" }
    }
}

/// Was nach „Änderungen verwerfen?“ geschieht.
#[derive(Debug)]
enum After {
    Close,
    Open(PathBuf),
    New(Box<Theater>, PathBuf),
}

#[derive(Debug)]
enum Dialog {
    None,
    Path(Ask, String),
    New { name: String, seed: String, path: String },
    Seed(String),
    Discard(After),
}

/// Ein laufender Pinselstrich: Stand davor und letzte Rasterlage.
#[derive(Debug)]
struct Drawing {
    before: Theater,
    last: [f32; 2],
}

pub struct Editor {
    theater: Theater,
    path: PathBuf,
    /// Stand der Datei auf der Platte; `None`: noch nie gespeichert.
    saved: Option<Theater>,
    history: History,
    drawing: Option<Drawing>,
    /// Rasterlage unter der Maus (Zeile, Spalte).
    hover: Option<[f32; 2]>,
    layer: Layer,
    ground: Ground,
    side: Side,
    tool: Tool,
    size: usize,
    zoom: f32,
    /// Lage der Kartenecke (Nordwest) im Kartenfeld.
    offset: Vec2,
    fit: bool,
    landscape: bool,
    show_sides: bool,
    show_grid: bool,
    /// Raster, aus dem die beiden Zelltexturen (Gelände, Seite) stammen.
    cells: Option<(Grid, TextureHandle, TextureHandle)>,
    worker: preview::Worker,
    preview: Option<TextureHandle>,
    preview_of: Option<Theater>,
    /// Unterlage und ihre Datei.
    underlay: Option<(TextureHandle, PathBuf)>,
    recent: Recent,
    show_underlay: bool,
    underlay_alpha: f32,
    dialog: Dialog,
    message: String,
    allow_close: bool,
    title: String,
}

impl Editor {
    /// Öffnet `file` (fehlt die Datei, ein neues Gebiet dieses Namens; ohne
    /// Angabe die zuletzt bearbeitete Datei) samt gemerkter oder angegebener
    /// Unterlage.
    pub fn new(
        ctx: &egui::Context,
        storage: Option<&dyn eframe::Storage>,
        file: Option<PathBuf>,
        underlay: Option<PathBuf>,
    ) -> Self {
        let repaint = ctx.clone();
        let recent = Recent::load(storage);
        let file = file.or_else(|| recent.file.clone().filter(|f| f.exists()));
        let path = file.clone().unwrap_or_else(|| Path::new(THEATER_DIR).join("neu.ron"));
        let name = path.file_stem().map_or("NEU".into(), |s| s.to_string_lossy().to_uppercase());
        let mut e = Self {
            theater: Theater::empty(&name, new_seed()),
            path,
            saved: None,
            history: History::default(),
            drawing: None,
            hover: None,
            layer: Layer::Ground,
            ground: Ground::Water,
            side: Side::Hostile,
            tool: Tool::Brush,
            size: 3,
            zoom: 6.0,
            offset: Vec2::ZERO,
            fit: true,
            landscape: false,
            show_sides: false,
            show_grid: true,
            cells: None,
            worker: preview::Worker::new(preview::SIZE, move || repaint.request_repaint()),
            preview: None,
            preview_of: None,
            underlay: None,
            recent,
            show_underlay: true,
            underlay_alpha: 0.5,
            dialog: Dialog::None,
            message: String::new(),
            allow_close: false,
            title: String::new(),
        };
        match file {
            Some(f) if f.exists() => e.open(ctx, f),
            _ => e.message = format!("Neues Gebiet {name}, Seed {}", e.theater.seed),
        }
        if let Some(u) = underlay {
            e.load_underlay(ctx, &u);
        }
        e
    }

    fn dirty(&self) -> bool {
        self.saved.as_ref() != Some(&self.theater)
    }

    fn paint(&self) -> Paint {
        match self.layer {
            Layer::Ground => Paint::Ground(self.ground),
            Layer::Side => Paint::Side(self.side),
        }
    }

    fn open(&mut self, ctx: &egui::Context, path: PathBuf) {
        let read = std::fs::read_to_string(&path).map_err(|e| e.to_string());
        match read.and_then(|s| Theater::parse(&s).map_err(|e| e.to_string())) {
            Ok(t) => {
                self.message = format!("Geöffnet: {} ({}, Seed {})", path.display(), t.name, t.seed);
                self.saved = Some(t.clone());
                self.theater = t;
                self.path = path;
                self.history.clear();
                self.fit = true;
                self.underlay = None;
                if let Some(u) = self.recent.underlay(&self.path) {
                    self.load_underlay(ctx, &u);
                }
            }
            Err(e) => self.message = format!("Nicht geöffnet: {}: {e}", path.display()),
        }
    }

    /// Schreibt über `Theater::to_text` und nur, was `Theater::parse` wieder liest.
    fn save(&mut self) {
        let text = self.theater.to_text();
        let dir = self.path.parent().filter(|d| !d.as_os_str().is_empty());
        let result = Theater::parse(&text)
            .map_err(|e| e.to_string())
            .and_then(|_| dir.map_or(Ok(()), std::fs::create_dir_all).map_err(|e| e.to_string()))
            .and_then(|()| std::fs::write(&self.path, text).map_err(|e| e.to_string()));
        self.message = match result {
            Ok(()) => {
                self.saved = Some(self.theater.clone());
                if let Some((_, u)) = &self.underlay {
                    self.recent.set_underlay(&self.path, u);
                }
                format!("Gespeichert: {}", self.path.display())
            }
            Err(e) => format!("Nicht gespeichert: {}: {e}", self.path.display()),
        };
    }

    fn load_underlay(&mut self, ctx: &egui::Context, path: &Path) {
        match read_png(path) {
            Ok((size, rgba)) => {
                let img = ColorImage::from_rgba_unmultiplied(size, &rgba);
                self.underlay =
                    Some((ctx.load_texture("unterlage", img, TextureOptions::LINEAR), path.into()));
                self.show_underlay = true;
                self.recent.set_underlay(&self.path, path);
            }
            Err(e) => self.message = format!("Unterlage nicht geladen: {e}"),
        }
    }

    fn water_from_mask(&mut self, path: &Path) {
        match read_png(path) {
            Ok((size, rgba)) => {
                let gray: Vec<u8> = rgba
                    .chunks_exact(4)
                    .map(|p| ((u16::from(p[0]) + u16::from(p[1]) + u16::from(p[2])) / 3) as u8)
                    .collect();
                let before = self.theater.clone();
                let n = edit::water_from_mask(&mut self.theater, size, &gray);
                if n > 0 {
                    self.history.push(before);
                }
                self.message = format!("Wasser aus Maske: {n} Zellen geändert");
            }
            Err(e) => self.message = format!("Maske nicht gelesen: {e}"),
        }
    }

    /// Erst fragen, wenn ungespeicherte Änderungen verloren gingen.
    fn leave(&mut self, ctx: &egui::Context, after: After) {
        if self.dirty() {
            self.dialog = Dialog::Discard(after);
        } else {
            self.proceed(ctx, after);
        }
    }

    fn proceed(&mut self, ctx: &egui::Context, after: After) {
        match after {
            After::Close => {
                self.allow_close = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            After::Open(p) => self.open(ctx, p),
            After::New(t, p) => {
                self.message = format!("Neues Gebiet {}, Seed {}", t.name, t.seed);
                self.theater = *t;
                self.path = p;
                self.saved = None;
                self.history.clear();
                self.fit = true;
            }
        }
    }

    fn undo(&mut self) {
        self.history.undo(&mut self.theater);
    }

    fn redo(&mut self) {
        self.history.redo(&mut self.theater);
    }

    // ---------------------------------------------------------------- Bilder

    /// Zelltexturen und Landschaftsvorschau auf den Stand des Gebiets bringen.
    fn refresh(&mut self, ctx: &egui::Context) {
        if self.cells.as_ref().is_none_or(|(g, ..)| *g != self.theater.grid) {
            let t = &self.theater;
            let layer = |f: &dyn Fn(usize, usize) -> Color32| {
                let px = (0..GRID_N * GRID_N).map(|k| f(k / GRID_N, k % GRID_N)).collect();
                ColorImage::new([GRID_N, GRID_N], px)
            };
            let ground = layer(&|r, c| ground_color(t.ground(r, c)));
            let side = layer(&|r, c| side_color(t.side(r, c)));
            self.cells = Some((
                t.grid.clone(),
                ctx.load_texture("gelaende", ground, TextureOptions::NEAREST),
                ctx.load_texture("seiten", side, TextureOptions::NEAREST),
            ));
        }
        if let Some(rgb) = self.worker.poll() {
            let img = ColorImage::from_rgb([preview::SIZE; 2], &rgb);
            self.preview = Some(ctx.load_texture("landschaft", img, TextureOptions::LINEAR));
        }
        if self.landscape && !self.worker.busy && self.preview_of.as_ref() != Some(&self.theater) {
            self.worker.request(&self.theater);
            self.preview_of = Some(self.theater.clone());
        }
        let title = format!(
            "F-29 Karteneditor – {} ({}){}",
            self.theater.name,
            self.path.display(),
            if self.dirty() { " *" } else { "" }
        );
        if title != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    // --------------------------------------------------------------- Eingabe

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = |k: Key| ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, k));
        if cmd(Key::S) {
            self.save();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)) || cmd(Key::Y) {
            self.redo();
        }
        if cmd(Key::Z) {
            self.undo();
        }
        if ctx.egui_wants_keyboard_input() || !matches!(self.dialog, Dialog::None) {
            return;
        }
        let pressed = |k: Key| ctx.input(|i| i.key_pressed(k));
        let digits = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8];
        for (k, key) in digits.into_iter().enumerate() {
            if pressed(key) {
                match self.layer {
                    Layer::Ground => self.ground = Ground::ALL[k],
                    Layer::Side => self.side = *Side::ALL.get(k).unwrap_or(&self.side),
                }
            }
        }
        if pressed(Key::E) {
            self.layer = if self.layer == Layer::Ground { Layer::Side } else { Layer::Ground };
            self.show_sides = self.layer == Layer::Side;
        }
        if pressed(Key::B) {
            self.tool = Tool::Brush;
        }
        if pressed(Key::F) {
            self.tool = Tool::Fill;
        }
        if pressed(Key::L) {
            self.landscape = !self.landscape;
        }
        if pressed(Key::U) {
            self.show_underlay = !self.show_underlay;
        }
        let i = SIZES.iter().position(|&s| s == self.size).unwrap_or(1);
        if pressed(Key::OpenBracket) {
            self.size = SIZES[i.saturating_sub(1)];
        }
        if pressed(Key::CloseBracket) {
            self.size = SIZES[(i + 1).min(SIZES.len() - 1)];
        }
    }

    /// Gezogene Dateien: `.ron` öffnen, `.png` als Unterlage.
    fn dropped(&mut self, ctx: &egui::Context) {
        let files: Vec<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        for f in files {
            match f.extension().and_then(|x| x.to_str()) {
                Some("ron") => self.leave(ctx, After::Open(f)),
                Some("png") => self.load_underlay(ctx, &f),
                _ => self.message = format!("Unbekannte Datei: {}", f.display()),
            }
        }
    }

    // ---------------------------------------------------------------- Panels

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Neu …").clicked() {
                let path = self.path.with_file_name("neu.ron").display().to_string();
                self.dialog = Dialog::New { name: "NEU".into(), seed: new_seed().to_string(), path };
            }
            if ui.button("Öffnen …").clicked() {
                self.dialog = Dialog::Path(Ask::Open, self.path.display().to_string());
            }
            if ui.button("Speichern").on_hover_text("Strg+S").clicked() {
                self.save();
            }
            if ui.button("Speichern unter …").clicked() {
                self.dialog = Dialog::Path(Ask::SaveAs, self.path.display().to_string());
            }
            ui.separator();
            if ui
                .add_enabled(self.history.can_undo(), egui::Button::new("Rückgängig"))
                .on_hover_text("Strg+Z")
                .clicked()
            {
                self.undo();
            }
            if ui
                .add_enabled(self.history.can_redo(), egui::Button::new("Wiederholen"))
                .on_hover_text("Strg+Y")
                .clicked()
            {
                self.redo();
            }
            ui.separator();
            let star = if self.dirty() { " – ungespeichert" } else { "" };
            ui.label(format!("{}{star}", self.path.display()));
        });
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Gebiet");
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut self.theater.name);
        });
        ui.horizontal(|ui| {
            ui.label(format!("Seed {}", self.theater.seed));
            let free = self.theater.objects.is_empty();
            let hint = if free {
                "Formt Grate und Hügel; nur bewusst ändern"
            } else {
                "Nur ohne Objekte änderbar"
            };
            if ui.add_enabled(free, egui::Button::new("ändern …")).on_hover_text(hint).clicked() {
                self.dialog = Dialog::Seed(self.theater.seed.to_string());
            }
        });
        ui.separator();

        ui.heading("Malen");
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.layer, Layer::Ground, "Gelände");
            if ui.radio_value(&mut self.layer, Layer::Side, "Seite").clicked() {
                self.show_sides = true;
            }
        });
        let swatch = |ui: &mut egui::Ui, color: Color32, selected: bool, text: String| {
            ui.horizontal(|ui| {
                let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                ui.painter().rect_filled(r, 3.0, color);
                resp.clicked() | ui.selectable_label(selected, text).clicked()
            })
            .inner
        };
        match self.layer {
            Layer::Ground => {
                for (k, g) in Ground::ALL.into_iter().enumerate() {
                    let text = format!("{}  {}  ({})", g.to_char(), ground_name(g), k + 1);
                    if swatch(ui, ground_color(g), self.ground == g, text) {
                        self.ground = g;
                    }
                }
            }
            Layer::Side => {
                for (k, s) in Side::ALL.into_iter().enumerate() {
                    let text = format!("{}  {}  ({})", s.to_char(), side_name(s), k + 1);
                    if swatch(ui, side_color(s), self.side == s, text) {
                        self.side = s;
                    }
                }
            }
        }
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.tool, Tool::Brush, "Pinsel (B)");
            ui.radio_value(&mut self.tool, Tool::Fill, "Füllen (F)");
        });
        ui.horizontal(|ui| {
            ui.label("Größe");
            for s in SIZES {
                ui.selectable_value(&mut self.size, s, s.to_string());
            }
        });
        ui.separator();

        ui.heading("Ansicht");
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.landscape, false, "Zellen");
            ui.radio_value(&mut self.landscape, true, "Landschaft (L)");
        });
        ui.checkbox(&mut self.show_sides, "Seiten einfärben");
        ui.checkbox(&mut self.show_grid, "Planquadrate (100 km)");
        if ui.button("Einpassen").clicked() {
            self.fit = true;
        }
        ui.separator();

        ui.heading("Unterlage");
        ui.horizontal(|ui| {
            if ui.button("Laden …").clicked() {
                self.dialog = Dialog::Path(Ask::Underlay, self.dir_of_path());
            }
            if let Some(name) = self.underlay.as_ref().and_then(|(_, p)| p.file_name()) {
                ui.label(name.to_string_lossy());
            }
        });
        if self.underlay.is_some() {
            ui.checkbox(&mut self.show_underlay, "zeigen (U)");
            ui.add(egui::Slider::new(&mut self.underlay_alpha, 0.0..=1.0).text("Deckkraft"));
        }
        if ui.button("Wasser aus Maske …").clicked() {
            self.dialog = Dialog::Path(Ask::Mask, self.dir_of_path());
        }
        ui.separator();

        ui.label(
            "Linke Maustaste malt, rechte nimmt den Wert unter der Maus auf. \
             Mausrad zoomt, mittlere Maustaste oder Leertaste + Ziehen verschiebt. \
             E wechselt die Ebene, [ und ] die Pinselgröße. \
             Dateien (.ron, .png) lassen sich ins Fenster ziehen.",
        );
    }

    /// Ordner der Gebietsdatei als Startpunkt für Dateiabfragen.
    fn dir_of_path(&self) -> String {
        let dir = self.path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
        format!("{}/", dir.display())
    }

    // ----------------------------------------------------------------- Karte

    fn map_rect(&self, canvas: Rect) -> Rect {
        Rect::from_min_size(canvas.min + self.offset, Vec2::splat(GRID_N as f32 * self.zoom))
    }

    /// Bildschirmpunkt eines Weltpunkts (Nord, Ost).
    fn screen(map: Rect, north: f64, east: f64) -> Pos2 {
        let s = f64::from(map.width()) / SIZE_M;
        pos2(map.min.x + ((east + SIZE_M * 0.5) * s) as f32, map.min.y + ((SIZE_M * 0.5 - north) * s) as f32)
    }

    fn canvas(&mut self, ui: &mut egui::Ui) -> Option<[f32; 2]> {
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        if self.fit {
            self.zoom = ((rect.width().min(rect.height()) - 40.0) / GRID_N as f32).clamp(ZOOM_MIN, ZOOM_MAX);
            self.offset = (rect.size() - Vec2::splat(GRID_N as f32 * self.zoom)) * 0.5;
            self.fit = false;
        }
        // Zoomen um den Mauszeiger, Verschieben
        if let Some(p) = resp.hover_pos() {
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let z = (self.zoom * (scroll / 300.0).exp() * pinch).clamp(ZOOM_MIN, ZOOM_MAX);
            if z != self.zoom {
                let at = p - rect.min;
                self.offset = at - (at - self.offset) * (z / self.zoom);
                self.zoom = z;
            }
        }
        let space = ui.input(|i| i.key_down(Key::Space));
        if resp.dragged_by(PointerButton::Middle) || (space && resp.dragged_by(PointerButton::Primary)) {
            self.offset += resp.drag_delta();
        }
        let map = self.map_rect(rect);
        let at = |p: Pos2| [(p.y - map.min.y) / self.zoom, (p.x - map.min.x) / self.zoom];
        let inside = |c: [f32; 2]| c.iter().all(|v| (0.0..GRID_N as f32).contains(v));
        let hover = resp.hover_pos().map(at);

        // Malen
        let paint = self.paint();
        let down = !space && resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down());
        let pointer = resp.interact_pointer_pos().map(at);
        // Auch ein kurzer Klick (Drücken und Loslassen im selben Bild) malt.
        let click = !space && resp.clicked_by(PointerButton::Primary);
        match (self.tool, down || click, pointer) {
            (Tool::Brush, true, Some(c)) => {
                let d = self.drawing.get_or_insert_with(|| Drawing { before: self.theater.clone(), last: c });
                edit::stroke(&mut self.theater, paint, d.last, c, self.size);
                d.last = c;
            }
            (Tool::Fill, _, Some(c)) if click && inside(c) => {
                let before = self.theater.clone();
                if edit::fill(&mut self.theater, paint, c[0] as usize, c[1] as usize) > 0 {
                    self.history.push(before);
                }
            }
            _ => {}
        }
        if !down
            && let Some(d) = self.drawing.take()
            && d.before != self.theater
        {
            self.history.push(d.before);
        }
        // Wert aufnehmen
        if resp.secondary_clicked()
            && let Some(c) = hover.filter(|&c| inside(c))
        {
            match paint.at(&self.theater, c[0] as usize, c[1] as usize) {
                Paint::Ground(g) => self.ground = g,
                Paint::Side(s) => self.side = s,
            }
        }

        self.draw(ui, rect, map, hover.filter(|&c| inside(c)));
        hover.filter(|&c| inside(c))
    }

    fn draw(&self, ui: &egui::Ui, rect: Rect, map: Rect, hover: Option<[f32; 2]>) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_gray(28));
        let uv = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
        let Some((_, ground, side)) = &self.cells else { return };
        let base = match (&self.preview, self.landscape) {
            (Some(p), true) => p.id(),
            _ => ground.id(),
        };
        painter.image(base, map, uv, Color32::WHITE);
        if self.show_sides {
            let a = if self.layer == Layer::Side { 150 } else { 80 };
            painter.image(side.id(), map, uv, Color32::from_white_alpha(a));
        }
        if let Some((tex, _)) = self.underlay.as_ref().filter(|_| self.show_underlay) {
            painter.image(tex.id(), map, uv, Color32::from_white_alpha((self.underlay_alpha * 255.0) as u8));
        }
        // Zellgitter beim Hineinzoomen
        if self.zoom >= 12.0 {
            let stroke = (1.0, Color32::from_black_alpha(40));
            for k in 0..=GRID_N {
                let d = k as f32 * self.zoom;
                painter
                    .line_segment([pos2(map.min.x + d, map.min.y), pos2(map.min.x + d, map.max.y)], stroke);
                painter
                    .line_segment([pos2(map.min.x, map.min.y + d), pos2(map.max.x, map.min.y + d)], stroke);
            }
        }
        // Planquadrate: Linien auf vollen 100 km, Buchstaben von West bzw. Süd
        if self.show_grid {
            let stroke = (1.0, Color32::from_white_alpha(110));
            let font = FontId::proportional(14.0);
            for k in -2..=2 {
                let v = f64::from(k) * 100_000.0;
                painter.line_segment(
                    [Self::screen(map, SIZE_M * 0.5, v), Self::screen(map, -SIZE_M * 0.5, v)],
                    stroke,
                );
                painter.line_segment(
                    [Self::screen(map, v, -SIZE_M * 0.5), Self::screen(map, v, SIZE_M * 0.5)],
                    stroke,
                );
            }
            for k in 0..6 {
                let a = (-300_000.0 + f64::from(k) * 100_000.0).max(-SIZE_M * 0.5);
                let b = (-200_000.0 + f64::from(k) * 100_000.0).min(SIZE_M * 0.5);
                let letter = char::from(b'A' + k as u8).to_string();
                let top = Self::screen(map, SIZE_M * 0.5, (a + b) * 0.5) - vec2(0.0, 3.0);
                let left = Self::screen(map, (a + b) * 0.5, -SIZE_M * 0.5) - vec2(5.0, 0.0);
                painter.text(top, Align2::CENTER_BOTTOM, &letter, font.clone(), Color32::LIGHT_GRAY);
                painter.text(left, Align2::RIGHT_CENTER, &letter, font.clone(), Color32::LIGHT_GRAY);
            }
        }
        // Flugplätze und Objekte (Bearbeiten folgt)
        for a in &self.theater.airfields {
            let c = if a.kind == AirfieldKind::Home {
                Color32::from_rgb(40, 120, 255)
            } else {
                Color32::from_rgb(210, 0, 210)
            };
            let p = Self::screen(map, a.north, a.east);
            painter.rect_filled(Rect::from_center_size(p, Vec2::splat(9.0)), 1.0, c);
            if self.zoom >= 4.0 {
                painter.text(
                    p + vec2(8.0, 0.0),
                    Align2::LEFT_CENTER,
                    &a.name,
                    FontId::proportional(12.0),
                    Color32::WHITE,
                );
            }
        }
        for o in &self.theater.objects {
            let c = if o.civilian {
                Color32::WHITE
            } else if o.class.is_radar() {
                Color32::YELLOW
            } else {
                Color32::RED
            };
            painter.circle_filled(Self::screen(map, o.north, o.east), 3.0, c);
        }
        // Pinsel
        if let Some(h) = hover.filter(|_| self.tool == Tool::Brush) {
            let center = map.min + vec2(h[1].floor() + 0.5, h[0].floor() + 0.5) * self.zoom;
            let r = (self.size as f32 * 0.5).max(0.5) * self.zoom;
            painter.circle_stroke(center, r, (1.0, Color32::WHITE));
        }
    }

    fn status(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some([r, c]) = self.hover {
                let north = SIZE_M * 0.5 - f64::from(r) * CELL_M;
                let east = f64::from(c) * CELL_M - SIZE_M * 0.5;
                let (row, col) = (r as usize, c as usize);
                let mut text = format!(
                    "Zeile {row}, Spalte {col}   N {:+.1} km  O {:+.1} km   {}   {}, {}",
                    north / 1e3,
                    east / 1e3,
                    grid_square(north, east, SIZE_M * 0.5),
                    ground_name(self.theater.ground(row, col)),
                    side_name(self.theater.side(row, col)),
                );
                // Objekt oder Flugplatz unter der Maus (bis 8 Pixel)
                let near = 8.0 * SIZE_M / (GRID_N as f64 * f64::from(self.zoom));
                let d = |n: f64, e: f64| (n - north).hypot(e - east);
                let airfield = self.theater.airfields.iter().map(|a| (d(a.north, a.east), a.name.clone()));
                let objects = self.theater.objects.iter().map(|o| (d(o.north, o.east), o.label()));
                if let Some((_, name)) =
                    airfield.chain(objects).filter(|x| x.0 < near).min_by(|a, b| a.0.total_cmp(&b.0))
                {
                    text += &format!("   {name}");
                }
                ui.label(text);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.landscape && (self.worker.busy || self.preview_of.as_ref() != Some(&self.theater)) {
                    ui.spinner();
                }
                ui.label(&self.message);
            });
        });
    }

    // --------------------------------------------------------------- Dialoge

    fn dialogs(&mut self, ctx: &egui::Context) {
        let mut dialog = std::mem::replace(&mut self.dialog, Dialog::None);
        let (mut ok, mut cancel) = (false, false);
        let mut buttons = |ui: &mut egui::Ui, ok_text: &str| {
            ui.horizontal(|ui| {
                ok = ui.button(ok_text).clicked() || ui.input(|i| i.key_pressed(Key::Enter));
                cancel = ui.button("Abbrechen").clicked();
            });
        };
        let modal = egui::Modal::new(egui::Id::unique("dialog"));
        let closed = match &mut dialog {
            Dialog::None => return,
            Dialog::Path(ask, path) => modal.show(ctx, |ui| {
                ui.heading(ask.title());
                focus(ui.add(egui::TextEdit::singleline(path).desired_width(520.0)));
                file_list(ui, path, ask.ext());
                buttons(ui, "OK");
            }),
            Dialog::New { name, seed, path } => modal.show(ctx, |ui| {
                ui.heading("Neues Gebiet");
                egui::Grid::new("neu").show(ui, |ui| {
                    ui.label("Name");
                    focus(ui.add(egui::TextEdit::singleline(name).desired_width(420.0)));
                    ui.end_row();
                    ui.label("Seed");
                    ui.add(egui::TextEdit::singleline(seed).desired_width(420.0));
                    ui.end_row();
                    ui.label("Datei");
                    ui.add(egui::TextEdit::singleline(path).desired_width(420.0));
                    ui.end_row();
                });
                ui.label("Der Seed formt die Feinheiten der Landschaft und bleibt danach fest.");
                buttons(ui, "Anlegen");
            }),
            Dialog::Seed(seed) => modal.show(ctx, |ui| {
                ui.heading("Seed ändern");
                ui.label(
                    "Ein neuer Seed verschiebt Grate und Hügel überall im Gebiet. \
                     Das geht nur, solange keine Objekte stehen; alte Replays dieses \
                     Gebiets werden ungültig.",
                );
                focus(ui.text_edit_singleline(seed));
                buttons(ui, "Ändern");
            }),
            Dialog::Discard(_) => modal.show(ctx, |ui| {
                ui.heading("Ungespeicherte Änderungen");
                ui.label("Die Änderungen an diesem Gebiet gehen verloren.");
                buttons(ui, "Verwerfen");
            }),
        }
        .should_close();
        if !ok {
            if !(cancel || closed) {
                self.dialog = dialog;
            }
            return;
        }
        match dialog {
            Dialog::None => {}
            Dialog::Path(ask, path) => {
                let p = PathBuf::from(path.trim());
                match ask {
                    Ask::Open => self.leave(ctx, After::Open(p)),
                    Ask::SaveAs => {
                        self.path = p;
                        self.save();
                    }
                    Ask::Underlay => self.load_underlay(ctx, &p),
                    Ask::Mask => self.water_from_mask(&p),
                }
            }
            Dialog::New { name, seed, path } => match seed.trim().parse() {
                Ok(seed) => {
                    let t = Box::new(Theater::empty(name.trim(), seed));
                    self.leave(ctx, After::New(t, PathBuf::from(path.trim())));
                }
                Err(_) => {
                    self.message = format!("Seed „{seed}“ ist keine Zahl");
                    self.dialog = Dialog::New { name, seed, path };
                }
            },
            Dialog::Seed(seed) => match seed.trim().parse() {
                Ok(s) if s != self.theater.seed => {
                    self.history.push(self.theater.clone());
                    self.theater.seed = s;
                    self.message = format!("Seed geändert auf {s}");
                }
                Ok(_) => {}
                Err(_) => self.message = format!("Seed „{seed}“ ist keine Zahl"),
            },
            Dialog::Discard(after) => self.proceed(ctx, after),
        }
    }
}

/// Gibt dem ersten Eingabefeld eines Dialogs den Fokus, solange keines ihn hat.
fn focus(r: egui::Response) {
    if r.ctx.memory(|m| m.focused().is_none()) {
        r.request_focus();
    }
}

/// Dateien mit Endung `ext` und Unterordner im Ordner von `path` zum Anklicken.
fn file_list(ui: &mut egui::Ui, path: &mut String, ext: &str) {
    let p = Path::new(path.as_str());
    let dir = if p.is_dir() {
        p.to_path_buf()
    } else {
        p.parent().filter(|d| d.is_dir()).unwrap_or(Path::new(".")).into()
    };
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() || p.extension().is_some_and(|x| x == ext))
        .collect();
    entries.sort();
    let up = dir.canonicalize().ok().and_then(|d| d.parent().map(Path::to_path_buf));
    egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
        if let Some(up) = up
            && ui.selectable_label(false, "../").clicked()
        {
            *path = format!("{}/", up.display());
        }
        for e in entries {
            let name = e.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            let dir = e.is_dir();
            if ui.selectable_label(false, if dir { format!("{name}/") } else { name }).clicked() {
                *path = e.display().to_string();
                if dir {
                    path.push('/');
                }
            }
        }
    });
}

impl eframe::App for Editor {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.recent.file = Some(absolute(&self.path));
        self.recent.store(storage);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty() && !self.allow_close {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.dialog = Dialog::Discard(After::Close);
        }
        self.shortcuts(&ctx);
        self.dropped(&ctx);
        self.refresh(&ctx);
        egui::Panel::top("datei").show(ui, |ui| self.top_bar(ui));
        // Die Statuszeile zeigt die Lage aus dem letzten Bild (die Karte kommt danach).
        egui::Panel::bottom("status").show(ui, |ui| self.status(ui));
        egui::Panel::left("werkzeug").default_size(250.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.side_panel(ui));
        });
        egui::CentralPanel::no_frame().show(ui, |ui| self.hover = self.canvas(ui));
        self.dialogs(&ctx);
    }
}
