//! Karteneditor für die Einsatzgebiete von F-29 Silent Sniper (M-031).
//!
//! Liest und schreibt Gebietsdateien nur über `f29_world::Theater`
//! (`docs/GEBIETSFORMAT.md`); die Landschaftsvorschau rechnet derselbe Code
//! wie im Spiel. Bedienung: `docs/KARTENEDITOR.md`.
//!
//! ```text
//! f29-editor [DATEI.ron] [--unterlage BILD.png]
//! ```
//!
//! Ohne Angabe öffnet er die zuletzt bearbeitete Datei mit ihrer Unterlage.

mod app;
mod edit;
mod objects;
mod preview;
mod recent;

use eframe::egui;
use std::path::PathBuf;

const USAGE: &str = "f29-editor [DATEI.ron] [--unterlage BILD.png]\n\
    Öffnet DATEI.ron (fehlt sie, ein neues Gebiet dieses Namens). Ohne Angabe\n\
    die zuletzt bearbeitete Datei mit ihrer Unterlage, beim ersten Start\n\
    data/theaters/neu.ron. --unterlage legt ein Kartenbild über das Gebiet.";

fn main() -> eframe::Result {
    let (mut file, mut underlay) = (None, None);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--unterlage" => underlay = args.next().map(PathBuf::from),
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ => file = Some(PathBuf::from(a)),
        }
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 950.0])
            .with_title("F-29 Karteneditor")
            // Name des Speicherordners (zuletzt bearbeitet, Fenstergröße)
            .with_app_id("f29-editor"),
        ..Default::default()
    };
    eframe::run_native(
        "F-29 Karteneditor",
        options,
        Box::new(move |cc| Ok(Box::new(app::Editor::new(&cc.egui_ctx, cc.storage, file, underlay)))),
    )
}
