//! `f29-headless` – Simulation ohne Grafik.
//!
//! ```text
//! f29-headless demo     [--seed N] [--seconds S] [--record DATEI]
//! f29-headless replay   DATEI
//! f29-headless hud-svg  [--seed N] [--seconds S] [--aspect A] [--page SEITE] [--radar 1] [--drop 1 [--after S]] [--damage 1] [--bay N] [--zoom 0..4] [--out DATEI]
//! f29-headless terrain  [--seed N] [--size PX] [--extent M] [--out DATEI.ppm]
//! f29-headless airfields [--seed N]
//! f29-headless sound    [--out VERZEICHNIS]
//! f29-headless bench    [--seconds S]
//! ```

use f29_core::autopilot::Autopilot;
use f29_core::controls::flags;
use f29_core::replay::Replay;
use f29_core::{DT, Sim, SimConfig, Status, TICK_HZ, detmath, units};
use f29_hud::{HudOptions, ScreenLayout};
use f29_terrain::Surface;
use std::process::ExitCode;

mod sound;
use std::time::Instant;

const USAGE: &str = "\
f29-headless – F-29 Silent Sniper ohne Grafik

Befehle:
  demo     [--seed N] [--seconds S] [--record DATEI]   Autopilot fliegt die Route ab
  replay   DATEI                                       Replay abspielen und Hash prüfen
  hud-svg  [--seed N] [--seconds S] [--aspect A] [--page SEITE] [--radar 1] [--drop 1 [--after S]] [--damage 1] [--bay N] [--zoom 0..4] [--out DATEI]
                                                       HUD nach S Sekunden Demo als SVG
                                                       (SEITE des rechten MFDs: msn, tgt, wpn, eng, adi, sys;
                                                       --radar 1 schaltet am Ende das Radar ein,
                                                       --drop 1 öffnet den Schacht und löst eine Waffe aus,
                                                       --damage 1 beschädigt Beispielsysteme,
                                                       --bay N wählt Waffenschacht N,
                                                       --zoom Kartenstufe 0..4 = 5..80 km)
  terrain  [--seed N] [--size PX] [--extent M] [--out DATEI.ppm]
                                                       Höhenkarte als PPM-Bild
  airfields [--seed N]                                 Flugplätze und Route auflisten
  sound    [--out VERZEICHNIS]                         Klänge als WAV-Hörproben rendern
  bench    [--seconds S]                               Ticks pro Sekunde messen
";

struct Args {
    positional: Vec<String>,
    options: Vec<(String, String)>,
}

impl Args {
    fn parse(raw: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut positional = Vec::new();
        let mut options = Vec::new();
        let mut it = raw.peekable();
        while let Some(a) = it.next() {
            if let Some(name) = a.strip_prefix("--") {
                let v = it.next().ok_or_else(|| format!("--{name} braucht einen Wert"))?;
                options.push((name.to_string(), v));
            } else {
                positional.push(a);
            }
        }
        Ok(Self { positional, options })
    }

    fn get<T: std::str::FromStr>(&self, name: &str, default: T) -> Result<T, String> {
        match self.options.iter().rev().find(|(k, _)| k == name) {
            None => Ok(default),
            Some((_, v)) => {
                let v = v.strip_prefix("0x").map_or_else(
                    || v.clone(),
                    |hex| u64::from_str_radix(hex, 16).map_or(v.clone(), |n| n.to_string()),
                );
                v.parse().map_err(|_| format!("ungültiger Wert für --{name}: {v}"))
            }
        }
    }

    fn opt(&self, name: &str) -> Option<&str> {
        self.options.iter().rev().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
}

fn main() -> ExitCode {
    let mut raw = std::env::args().skip(1);
    let Some(cmd) = raw.next() else {
        eprint!("{USAGE}");
        return ExitCode::from(2);
    };
    let args = match Args::parse(raw) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Fehler: {e}");
            return ExitCode::from(2);
        }
    };
    let result = match cmd.as_str() {
        "demo" => demo(&args),
        "replay" => replay(&args),
        "hud-svg" => hud_svg(&args),
        "terrain" => terrain(&args),
        "bench" => bench(&args),
        "airfields" => airfields(&args),
        "sound" => sound(&args),
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
            Ok(())
        }
        other => Err(format!("unbekannter Befehl `{other}`\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Fehler: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Lässt den Autopiloten die generierte Route abfliegen.
fn run_demo(seed: u64, seconds: f64, mut on_tick: impl FnMut(&Sim, f29_core::ControlFrame)) -> Sim {
    let mut sim = Sim::new(SimConfig::new(seed));
    let fd = sim.flight_data();
    let mut ap = Autopilot::new(fd.altitude_m, fd.heading, 220.0);
    let total = (seconds * f64::from(TICK_HZ)).round() as u64;
    for _ in 0..total {
        let fd = sim.flight_data();
        let mut next = false;
        if let Some(sp) = &fd.steerpoint {
            ap.target_heading = sp.bearing;
            // Sicherheitshöhe über dem höchsten Gelände bis 8 km voraus halten,
            // in Flugrichtung und in Richtung des Navigationspunkts (Kurven).
            let ahead = [fd.heading, sp.bearing]
                .into_iter()
                .flat_map(|dir| (1..=8).map(move |k| (dir, f64::from(k) * 1_000.0)))
                .map(|(dir, d)| {
                    sim.terrain().elevation(
                        fd.position.x + d * detmath::cos(dir),
                        fd.position.y + d * detmath::sin(dir),
                    )
                })
                .fold(f64::MIN, f64::max);
            ap.target_altitude_m = ahead.max(fd.ground_elevation_m) + 600.0;
            next = sp.range < 1_500.0;
        }
        let mut c = ap.control(sim.spec(), &fd, DT);
        if next {
            c.flags |= flags::NEXT_STEERPOINT;
        }
        sim.step(c);
        on_tick(&sim, c);
        if sim.status() != Status::Flying {
            break;
        }
    }
    sim
}

fn print_summary(sim: &Sim) {
    let fd = sim.flight_data();
    println!("Zeit         {:.1} s ({} Ticks)", fd.time_s, fd.tick);
    println!("Status       {:?}", fd.status);
    println!(
        "Position     N {:.0} m  E {:.0} m  Höhe {:.0} m (Radar {:.0} m)",
        fd.position.x, fd.position.y, fd.altitude_m, fd.radar_altitude_m
    );
    println!(
        "Flug         {:.0} km/h  Mach {:.2}  Kurs {:03.0}°  {:.1} g",
        units::mps_to_kmh(fd.tas_mps),
        fd.mach,
        fd.heading.to_degrees(),
        fd.load_factor
    );
    if let Some(sp) = &fd.steerpoint {
        println!("Steerpoint   {} in {:.1} km", sp.name, units::m_to_km(sp.range));
    }
    println!(
        "Treibstoff   {:.0} kg ({:.0} %), Restflugzeit {:.0} min",
        fd.fuel_kg,
        fd.fuel_fraction * 100.0,
        fd.endurance_s / 60.0
    );
    println!("Zustands-Hash {:016x}", sim.state_hash());
}

fn demo(args: &Args) -> Result<(), String> {
    let seed = args.get("seed", 0x0F29_u64)?;
    let seconds = args.get("seconds", 300.0_f64)?;
    let mut replay = Replay::new(SimConfig::new(seed));
    let mut last_report = 0;
    let sim = run_demo(seed, seconds, |sim, c| {
        replay.push(c);
        if sim.tick() - last_report >= u64::from(TICK_HZ) * 30 {
            last_report = sim.tick();
            let fd = sim.flight_data();
            println!(
                "t={:5.0}s  {:>6.0} m  {:>5.0} km/h  Kurs {:03.0}  {}",
                fd.time_s,
                fd.altitude_m,
                units::mps_to_kmh(fd.tas_mps),
                fd.heading.to_degrees(),
                fd.steerpoint.as_ref().map_or(String::new(), |s| format!(
                    "{} {:.1} km",
                    s.name,
                    units::m_to_km(s.range)
                ))
            );
        }
    });
    println!();
    print_summary(&sim);
    if let Some(path) = args.opt("record") {
        replay.final_hash = Some(sim.state_hash());
        std::fs::write(path, replay.to_text()).map_err(|e| format!("{path}: {e}"))?;
        println!("Replay gespeichert: {path} ({} Ticks)", replay.tick_count());
    }
    Ok(())
}

fn replay(args: &Args) -> Result<(), String> {
    let path = args.positional.first().ok_or("Pfad zur Replay-Datei fehlt")?;
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let replay = Replay::from_text(&text).map_err(|e| e.to_string())?;
    let start = Instant::now();
    let sim = replay.run();
    let elapsed = start.elapsed();
    print_summary(&sim);
    println!("Abgespielt   {} Ticks in {:.1} ms", replay.tick_count(), elapsed.as_secs_f64() * 1e3);
    match replay.final_hash {
        Some(h) if h == sim.state_hash() => {
            println!("Hash         OK");
            Ok(())
        }
        Some(h) => Err(format!("Hash weicht ab: erwartet {h:016x}, erhalten {:016x}", sim.state_hash())),
        None => {
            println!("Hash         (keiner gespeichert)");
            Ok(())
        }
    }
}

fn hud_svg(args: &Args) -> Result<(), String> {
    let seed = args.get("seed", 0x0F29_u64)?;
    let seconds = args.get("seconds", 20.0_f64)?;
    let aspect = args.get("aspect", 16.0_f32 / 9.0)?;
    let out = args.opt("out").unwrap_or("hud.svg");
    let page = args.opt("page").unwrap_or("msn");
    let right_page = f29_hud::RightPage::ALL
        .into_iter()
        .find(|p| p.label().eq_ignore_ascii_case(page))
        .ok_or_else(|| format!("unbekannte Seite {page} (msn, tgt, wpn, eng, adi, sys)"))?;
    let mut last = f29_core::ControlFrame::default();
    let mut sim = run_demo(seed, seconds, |_, c| last = c);
    if args.get("radar", 0_u8)? != 0 {
        sim.step(f29_core::ControlFrame { flags: last.flags | flags::RADAR_TOGGLE, ..last });
    }
    for _ in 1..args.get("bay", 1_usize)? {
        sim.step(last.with_weapons(f29_core::controls::weapon_flags::NEXT_BAY));
    }
    // Schacht öffnen, eine Waffe auslösen und kurz weiterfliegen (Meldung, Bestand).
    if args.get("drop", 0_u8)? != 0 {
        use f29_core::controls::weapon_flags;
        sim.step(last.with_weapons(weapon_flags::BAY_DOORS));
        for _ in 0..(f29_core::TICK_HZ * 3 / 2) {
            sim.step(last);
        }
        sim.step(last.with_weapons(weapon_flags::RELEASE));
        let after = args.get("after", 0.5_f64)?;
        for _ in 0..(after * f64::from(f29_core::TICK_HZ)) as u32 {
            sim.step(last);
        }
    }
    if args.get("damage", 0_u8)? != 0 {
        use f29_core::systems::{Condition, System};
        sim.set_condition(System::Engine, Condition::Degraded(0.6));
        sim.set_condition(System::Hydraulics, Condition::Degraded(0.35));
        sim.set_condition(System::Radar, Condition::Failed);
        sim.set_condition(System::WeaponBay, Condition::Failed);
    }
    let map = f29_hud::MapData::for_terrain(sim.terrain(), sim.route(), sim.targets());
    let layout = ScreenLayout::cockpit(aspect);
    let mut opts = HudOptions::default();
    opts.mfd.right_page = right_page;
    opts.mfd.map_zoom = args.get("zoom", opts.mfd.map_zoom)?;
    let dl = f29_hud::build(&sim.flight_data(), &layout, &opts, Some(&map));
    let svg = f29_hud::svg::to_svg(&dl, &layout, 720);
    std::fs::write(out, svg).map_err(|e| format!("{out}: {e}"))?;
    println!("{} Linien → {out}", dl.lines.len());
    Ok(())
}

fn terrain(args: &Args) -> Result<(), String> {
    let seed = args.get("seed", 0x0F29_u64)?;
    let size = args.get("size", 512_usize)?;
    let extent = args.get("extent", 120_000.0_f64)?;
    let out = args.opt("out").unwrap_or("terrain.ppm");
    let sim = Sim::new(SimConfig::new(seed));
    let t = sim.terrain();
    let mut img = format!("P6\n{size} {size}\n255\n").into_bytes();
    let route = sim.route().to_vec();
    for row in 0..size {
        for col in 0..size {
            // Norden oben, Osten rechts, Ursprung in der Bildmitte
            let n = (0.5 - (row as f64 + 0.5) / size as f64) * extent;
            let e = ((col as f64 + 0.5) / size as f64 - 0.5) * extent;
            let h = t.ground_height(n, e);
            let shade = {
                let nrm = t.normal(n, e);
                (0.55 + 0.45 * (-nrm[0] * 0.6 + nrm[1] * -0.4 + nrm[2] * 0.7)).clamp(0.3, 1.2)
            };
            let base: [f64; 3] = match t.surface(n, e) {
                Surface::Water => [40.0, 70.0 + (h / 30.0).max(-40.0), 150.0],
                Surface::Beach => [200.0, 190.0, 130.0],
                Surface::Grass => [90.0, 150.0, 70.0],
                Surface::Forest => [40.0, 105.0, 50.0],
                Surface::Rock => [130.0, 120.0, 110.0],
                Surface::Snow => [235.0, 235.0, 245.0],
            };
            let water = h < 0.0;
            for ch in base {
                let v = if water { ch } else { ch * shade };
                img.push(v.clamp(0.0, 255.0) as u8);
            }
        }
    }
    // Startpunkt und Route markieren
    let mut mark = |n: f64, e: f64, rgb: [u8; 3]| {
        let col = ((e / extent + 0.5) * size as f64) as i64;
        let row = ((0.5 - n / extent) * size as f64) as i64;
        let header = format!("P6\n{size} {size}\n255\n").len();
        for dr in -2..=2_i64 {
            for dc in -2..=2_i64 {
                let (r, c) = (row + dr, col + dc);
                if (0..size as i64).contains(&r) && (0..size as i64).contains(&c) {
                    let i = header + ((r as usize) * size + c as usize) * 3;
                    img[i..i + 3].copy_from_slice(&rgb);
                }
            }
        }
    };
    for sp in &route {
        mark(sp.north, sp.east, [255, 40, 40]);
    }
    // Bahnen als weiße Striche
    for rw in t.runways() {
        let steps = (rw.length_m / (extent / size as f64) * 2.0).max(4.0) as usize;
        for i in 0..=steps {
            let p = rw.world((i as f64 / steps as f64 - 0.5) * rw.length_m, 0.0);
            let col = ((p[1] / extent + 0.5) * size as f64) as i64;
            let row = ((0.5 - p[0] / extent) * size as f64) as i64;
            if (0..size as i64).contains(&row) && (0..size as i64).contains(&col) {
                let header = format!("P6\n{size} {size}\n255\n").len();
                let i = header + ((row as usize) * size + col as usize) * 3;
                img[i..i + 3].copy_from_slice(&[255, 255, 255]);
            }
        }
    }
    std::fs::write(out, img).map_err(|e| format!("{out}: {e}"))?;
    println!("{size}×{size} px, {:.0} km Kantenlänge → {out}", extent / 1000.0);
    Ok(())
}

fn airfields(args: &Args) -> Result<(), String> {
    let seed = args.get("seed", 0x0F29_u64)?;
    let sim = Sim::new(SimConfig::new(seed));
    println!("Seed {seed:#x}");
    for rw in sim.terrain().runways() {
        let (a, b) = rw.numbers();
        println!(
            "{:<8} {:?}  Bahn {a:02}/{b:02}  {:.0} × {:.0} m  Höhe {:.0} m  bei N {:.1} km  E {:.1} km",
            rw.name,
            rw.kind,
            rw.length_m,
            rw.width_m,
            rw.elevation_m,
            rw.center[0] / 1_000.0,
            rw.center[1] / 1_000.0
        );
    }
    println!("Route:");
    for sp in sim.route() {
        println!("  {:<8} N {:.1} km  E {:.1} km", sp.name, sp.north / 1_000.0, sp.east / 1_000.0);
    }
    Ok(())
}

fn sound(args: &Args) -> Result<(), String> {
    let dir = std::path::PathBuf::from(args.opt("out").unwrap_or("sounds"));
    for path in sound::render_all(&dir)? {
        println!("{path}");
    }
    Ok(())
}

fn bench(args: &Args) -> Result<(), String> {
    let seconds = args.get("seconds", 600.0_f64)?;
    let start = Instant::now();
    let sim = run_demo(0x0F29, seconds, |_, _| {});
    let dt = start.elapsed().as_secs_f64();
    let rate = sim.tick() as f64 / dt;
    println!(
        "{} Ticks in {:.3} s → {:.0} Ticks/s ({:.0}× Echtzeit)",
        sim.tick(),
        dt,
        rate,
        rate / f64::from(TICK_HZ)
    );
    Ok(())
}
