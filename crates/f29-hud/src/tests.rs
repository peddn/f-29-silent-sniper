use super::*;
use f29_core::{ControlFrame, Sim, SimConfig};

fn sample() -> FlightData {
    let mut sim = Sim::new(SimConfig::new(0x0F29));
    for _ in 0..120 {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.55, 0));
    }
    sim.flight_data()
}

fn bounds(dl: &DisplayList) -> ([f32; 2], [f32; 2]) {
    let mut min = [f32::MAX; 2];
    let mut max = [f32::MIN; 2];
    for l in &dl.lines {
        for p in [l.a, l.b] {
            for i in 0..2 {
                min[i] = min[i].min(p[i]);
                max[i] = max[i].max(p[i]);
            }
        }
    }
    (min, max)
}

#[test]
fn everything_stays_on_screen_for_common_aspects() {
    let fd = sample();
    for aspect in [4.0 / 3.0, 16.0 / 10.0, 16.0 / 9.0, 21.0 / 9.0] {
        let layout = ScreenLayout::cockpit(aspect);
        let dl = build(&fd, &layout, &HudOptions::default(), None);
        assert!(dl.lines.len() > 200, "zu wenig Symbolik: {}", dl.lines.len());
        let (min, max) = bounds(&dl);
        assert!(min[0] >= -aspect / 2.0 - 1e-4 && max[0] <= aspect / 2.0 + 1e-4, "x außerhalb bei {aspect}");
        assert!(min[1] >= -0.5 - 1e-4 && max[1] <= 0.5 + 1e-4, "y außerhalb bei {aspect}");
    }
}

#[test]
fn output_is_deterministic() {
    let fd = sample();
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let a = build(&fd, &layout, &HudOptions::default(), None);
    let b = build(&fd, &layout, &HudOptions::default(), None);
    assert_eq!(a.lines, b.lines);
}

#[test]
fn boresight_projects_to_view_center() {
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let p = layout.project_body([1.0, 0.0, 0.0]).unwrap();
    assert_eq!(p, layout.view_rect().center());
    assert!(layout.project_body([-1.0, 0.0, 0.0]).is_none());
    // Nach oben (−z in Körperachsen) ⇒ auf dem Schirm höher
    let up = layout.project_body([1.0, 0.0, -0.1]).unwrap();
    assert!(up[1] > p[1]);
    // Halber Sichtwinkel nach oben ⇒ Oberkante der Ansicht
    let half = (layout.vertical_fov as f64 * 0.5).tan();
    let edge = layout.project_body([1.0, 0.0, -half]).unwrap();
    assert!((edge[1] - 0.5).abs() < 1e-5);
}

#[test]
fn ladder_moves_down_when_nose_goes_up() {
    let mut fd = sample();
    fd.pitch = 0.0;
    fd.roll = 0.0;
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let c = layout.view_rect().center();
    // Horizontlinie: längste Linie, die nahe der Mitte waagrecht liegt
    let horizon_y = |fd: &FlightData| {
        let mut dl = DisplayList::new();
        hud::draw(&mut dl, fd, &layout);
        dl.lines
            .iter()
            .filter(|l| (l.a[1] - l.b[1]).abs() < 1e-6 && (l.b[0] - l.a[0]).abs() > 0.08)
            .map(|l| l.a[1])
            .next()
            .expect("Horizontlinie")
    };
    assert!((horizon_y(&fd) - c[1]).abs() < 1e-5);
    fd.pitch = 0.1;
    assert!(horizon_y(&fd) < c[1] - 0.01);
}

#[test]
fn crash_screen_replaces_hud() {
    let mut fd = sample();
    fd.warnings.crashed = true;
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let dl = build(&fd, &layout, &HudOptions { show_panel: false, ..HudOptions::default() }, None);
    assert!(dl.lines.iter().all(|l| l.ink == Ink::Warning));
}

#[test]
fn heading_formatting() {
    assert_eq!(heading_deg(0.0), 0);
    assert_eq!(heading_deg(std::f64::consts::TAU - 1e-6), 0);
    assert_eq!(heading_deg(std::f64::consts::FRAC_PI_2), 90);
}

#[test]
fn sink_marks_match_aircraft_limits() {
    let spec = &f29_core::data::F29_SPEC;
    let hard = f29_core::units::mps_to_fpm(spec.hard_landing_sink_mps) as f32;
    let max = f29_core::units::mps_to_fpm(spec.max_landing_sink_mps) as f32;
    assert!((crate::hud::HARD_SINK_FPM - hard).abs() < 2.0, "{hard}");
    assert!((crate::hud::MAX_SINK_FPM - max).abs() < 2.0, "{max}");
}

/// Endanflug mit ausgefahrenem Fahrwerk: Landehilfen erscheinen, alles bleibt im Bild.
#[test]
fn landing_aids_and_ground_messages_stay_on_screen() {
    use f29_core::controls::flags;
    use f29_core::sim::Spawn;
    let mut sim = Sim::new(SimConfig {
        seed: 0x0F29,
        spawn: Spawn {
            altitude: 600.0,
            min_clearance: 300.0,
            speed: 80.0,
            throttle: 0.5,
            ..Spawn::default()
        },
    });
    for _ in 0..(7 * 120) {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.5, flags::GEAR | flags::FLAPS));
    }
    let fd = sim.flight_data();
    assert!(fd.gear > 0.99);
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let with = build(&fd, &layout, &HudOptions::default(), None);
    let mut clean = fd.clone();
    clean.gear = 0.0;
    let without = build(&clean, &layout, &HudOptions::default(), None);
    assert!(with.lines.len() > without.lines.len() + 8, "keine Landehilfen");

    let mut ground = Sim::new(SimConfig::runway(0x0F29));
    ground.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.0, flags::GEAR));
    for aspect in [4.0 / 3.0, 16.0 / 9.0, 21.0 / 9.0] {
        let layout = ScreenLayout::cockpit(aspect);
        for fd in [fd.clone(), ground.flight_data()] {
            let dl = build(&fd, &layout, &HudOptions::default(), None);
            let (min, max) = bounds(&dl);
            assert!(min[0] >= -aspect / 2.0 - 1e-4 && max[0] <= aspect / 2.0 + 1e-4);
            assert!(min[1] >= -0.5 - 1e-4 && max[1] <= 0.5 + 1e-4);
        }
    }
}

#[test]
fn mfds_fit_in_panel_and_do_not_overlap() {
    for aspect in [4.0 / 3.0, 16.0 / 10.0, 16.0 / 9.0, 21.0 / 9.0] {
        let layout = ScreenLayout::cockpit(aspect);
        let m = layout.mfd_layout().expect("MFDs");
        let panel = layout.panel_rect();
        for r in [m.left, m.right, m.center] {
            assert!(
                r.min[0] >= panel.min[0] && r.max[0] <= panel.max[0],
                "breiter als das Brett bei {aspect}"
            );
            assert!(r.min[1] >= panel.min[1] && r.max[1] <= panel.max[1], "höher als das Brett bei {aspect}");
        }
        assert!(m.left.max[0] < m.center.min[0] && m.center.max[0] < m.right.min[0]);
        // quadratische Bildschirme, innerhalb der Gehäuse
        let w = m.left_screen.max[0] - m.left_screen.min[0];
        let h = m.left_screen.max[1] - m.left_screen.min[1];
        assert!((w - h).abs() < 1e-5);
        assert!(m.left.min[0] < m.left_screen.min[0] && m.left_screen.max[1] < m.left.max[1]);
    }
    // Ohne Brett (spätere Weitwinkelsicht) gibt es keine MFDs
    let wide = ScreenLayout { panel_fraction: 0.0, ..ScreenLayout::cockpit(16.0 / 9.0) };
    assert!(wide.mfd_layout().is_none());
    let dl = build(&sample(), &wide, &HudOptions::default(), None);
    assert!(dl.lines.iter().all(|l| l.a[1] > wide.view_rect().min[1] - 1e-4));
}

#[test]
fn map_draws_inside_left_screen_in_all_modes_and_zooms() {
    let mut sim = Sim::new(SimConfig::new(0x0F29));
    for _ in 0..120 {
        sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.55, 0));
    }
    let fd = sim.flight_data();
    let map = MapData::for_terrain(sim.terrain(), sim.route(), sim.targets());
    assert!(!map.contours.is_empty() && map.runways.len() >= 2 && map.route.len() >= 3);
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let m = layout.mfd_layout().unwrap();
    let panel_only = |opts: &HudOptions| {
        let dl = build(&fd, &layout, &HudOptions { show_hud: false, ..*opts }, Some(&map));
        dl.lines
    };
    let mut state = MfdState::default();
    let mut counts = Vec::new();
    for mode in [MapMode::Grid, MapMode::Satellite] {
        state.map_mode = mode;
        for zoom in 0..mfd::MAP_RANGES_NM.len() {
            state.map_zoom = zoom;
            let lines = panel_only(&HudOptions { mfd: state, ..HudOptions::default() });
            for l in &lines {
                for p in [l.a, l.b] {
                    let inside_left = p[0] >= m.left.min[0] - 1e-4 && p[0] <= m.left.max[0] + 1e-4;
                    if inside_left {
                        assert!(p[1] >= m.left.min[1] - 1e-4 && p[1] <= m.left.max[1] + 1e-4);
                    }
                }
            }
            counts.push((mode, zoom, lines.len()));
        }
    }
    // Gitterkarte zeichnet mehr Linien als die Satellitenansicht (Gitter, Höhenlinien)
    let grid: usize = counts.iter().filter(|c| c.0 == MapMode::Grid).map(|c| c.2).sum();
    let sat: usize = counts.iter().filter(|c| c.0 == MapMode::Satellite).map(|c| c.2).sum();
    assert!(grid > sat, "Gitter {grid} vs Satellit {sat}");
    // Zoomstufen sind begrenzt
    let mut s = MfdState::default();
    for _ in 0..10 {
        s.zoom_in();
    }
    assert_eq!(s.map_zoom, 0);
    for _ in 0..10 {
        s.zoom_out();
    }
    assert_eq!(s.map_zoom, mfd::MAP_RANGES_NM.len() - 1);
}

#[test]
fn fuel_bar_shrinks_with_fuel() {
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let m = layout.mfd_layout().unwrap();
    let fill_len = |fraction: f64| {
        let mut fd = sample();
        fd.fuel_kg *= fraction / fd.fuel_fraction;
        fd.fuel_fraction = fraction;
        // Der Treibstoffbalken steht auf der Triebwerksseite.
        let mfd = MfdState { right_page: RightPage::Engine, ..MfdState::default() };
        let dl = build(
            &fd,
            &layout,
            &HudOptions { show_hud: false, show_panel: true, mfd, camera_feed: false },
            None,
        );
        dl.lines
            .iter()
            .filter(|l| {
                (l.ink == Ink::Needle || l.ink == Ink::Warning)
                    && l.a[0] > m.right_screen.min[0]
                    && (l.a[1] - l.b[1]).abs() < 1e-6
            })
            .map(|l| (l.b[0] - l.a[0]).abs())
            .fold(0.0_f32, f32::max)
    };
    let full = fill_len(1.0);
    let half = fill_len(0.5);
    assert!(full > 0.1 && (half / full - 0.5).abs() < 0.02, "voll {full}, halb {half}");
}

/// Fahrwerk, Klappen und Bremse haben feste Zeilen: Blinken oder Ausblenden
/// einer Anzeige verschiebt die anderen nicht.
#[test]
fn configuration_rows_do_not_move() {
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let mut fd = sample();
    fd.flaps = 1.0;
    fd.airbrake = 1.0;
    fd.on_ground = false;
    // Unterkante der Textzeilen einer Tinte im linken unteren Bereich des Glases
    let row_y = |fd: &FlightData, ink: Ink, len: usize| {
        let mut dl = DisplayList::new();
        hud::draw(&mut dl, fd, &layout);
        let mut ys: Vec<f32> = dl
            .lines
            .iter()
            .filter(|l| l.ink == ink && l.a[0] < -0.17 && l.a[1] < layout.view_rect().center()[1] - 0.1)
            .map(|l| l.a[1].min(l.b[1]))
            .collect();
        ys.sort_by(f32::total_cmp);
        ys.dedup_by(|a, b| (*a - *b).abs() < 0.004);
        assert!(ys.len() >= len, "{ys:?}");
        ys
    };
    let lowest = |fd: &FlightData| row_y(fd, Ink::Hud, 1)[0];
    // Fahrwerk unten, zu schnell: blinkt rot – in beiden Phasen gleiche Bremszeile
    fd.gear = 1.0;
    fd.warnings.gear_overspeed = true;
    fd.time_s = 0.1; // sichtbar
    let with_gear = lowest(&fd);
    fd.time_s = 0.3; // ausgeblendet
    let without_gear = lowest(&fd);
    assert!((with_gear - without_gear).abs() < 1e-6, "AIRBRAKE rutscht: {with_gear} vs {without_gear}");
    // Fahrwerk eingefahren: Zeilen bleiben ebenfalls
    fd.gear = 0.0;
    fd.warnings.gear_overspeed = false;
    assert!((lowest(&fd) - with_gear).abs() < 1e-6, "AIRBRAKE rutscht bei eingefahrenem Fahrwerk");
}

#[test]
fn every_mfd_page_draws_inside_its_screen() {
    let fd = sample();
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let m = layout.mfd_layout().expect("Brett");
    let mut state = MfdState::default();
    let mut seen = Vec::new();
    for _ in 0..RightPage::ALL.len() {
        seen.push(state.right_page);
        let opts = HudOptions { show_hud: false, show_panel: true, mfd: state, camera_feed: false };
        let dl = build(&fd, &layout, &opts, None);
        let screen = m.right_screen;
        let inside = dl.lines.iter().filter(|l| screen.contains(l.a) && screen.contains(l.b)).count();
        assert!(inside > 60, "{:?}: kaum Inhalt ({inside} Linien)", state.right_page);
        state.next_page();
    }
    assert_eq!(state.right_page, RightPage::Mission, "F4 kehrt nach der letzten Seite zurück");
    seen.dedup();
    assert_eq!(seen.len(), RightPage::ALL.len(), "nicht alle Seiten erreicht");
}

/// Offene Schachtklappen blinken als Warnung im HUD.
#[test]
fn open_bay_blinks_as_warning() {
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let warn_lines = |door: f64, time_s: f64| {
        let mut fd = sample();
        fd.stores.door = door;
        fd.time_s = time_s;
        let dl = build(&fd, &layout, &HudOptions { show_panel: false, ..HudOptions::default() }, None);
        dl.lines.iter().filter(|l| l.ink == Ink::Warning).count()
    };
    let closed = warn_lines(0.0, 10.0);
    assert!(warn_lines(1.0, 10.0) > closed, "keine Warnung bei offenem Schacht");
    assert!(warn_lines(0.4, 10.0) > closed, "keine Warnung bei sich öffnendem Schacht");
    // Blinkphase aus: nichts zusätzlich
    assert_eq!(warn_lines(1.0, 10.4), closed);
}

/// Ausgefallene Anzeigen: HUD und MFD-Inhalte dunkel, Schubhebel bleibt;
/// beschädigte flackern.
#[test]
fn failed_displays_go_dark_and_damaged_ones_flicker() {
    use f29_core::systems::{Condition, System};
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let count = |cond: Condition, time_s: f64| {
        let mut fd = sample();
        fd.systems.set(System::Displays, cond);
        fd.time_s = time_s;
        build(&fd, &layout, &HudOptions::default(), None).lines.len()
    };
    let ok = count(Condition::Ok, 10.0);
    let dark = count(Condition::Failed, 10.0);
    assert!(dark > 0 && dark < ok / 4, "dunkel {dark} von {ok} Linien");
    let samples: Vec<usize> =
        (0..40).map(|i| count(Condition::Degraded(0.3), 10.0 + i as f64 * 0.013)).collect();
    assert!(samples.contains(&ok) && samples.contains(&dark), "flackert nicht");
}

/// Zielrahmen nach Reichweitenzone: Ecken in Reichweite, rot in der guten
/// Zone, gestrichelt ohne Wirkung.
#[test]
fn target_box_shows_the_launch_zone() {
    use f29_core::weapons::LaunchZone;
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let mut sim = Sim::new(SimConfig::new(0x0F29));
    sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.55, f29_core::controls::flags::RADAR_TOGGLE));
    let base = sim.flight_data();
    let Some(mut t) = base.target else { panic!("kein Ziel aufgeschaltet") };
    // Ziel genau voraus, damit der Rahmen im Glas liegt.
    t.direction_body = f29_core::DVec3::new(1.0, 0.0, 0.05).normalize_or_zero();
    t.civilian = false;
    let lines = |zone: Option<LaunchZone>| {
        let mut fd = base.clone();
        fd.target = Some(f29_core::TargetInfo { zone, ..t });
        let dl = build(&fd, &layout, &HudOptions { show_panel: false, ..HudOptions::default() }, None);
        (dl.lines.len(), dl.lines.iter().filter(|l| l.ink == Ink::Warning).count())
    };
    let (plain, plain_red) = lines(None);
    let (in_range, _) = lines(Some(LaunchZone::InRange));
    let (good, good_red) = lines(Some(LaunchZone::Good));
    let (none, _) = lines(Some(LaunchZone::NoEffect));
    assert_eq!(lines(Some(LaunchZone::OutOfRange)).0, plain);
    assert_eq!(in_range, plain + 8, "acht Eckstriche");
    assert!(good_red > plain_red + 8, "gute Zone rot");
    assert_eq!(good, in_range);
    assert_ne!(none, plain, "ohne Wirkung gestrichelt");
}
