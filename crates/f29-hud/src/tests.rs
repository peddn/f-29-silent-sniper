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
        hud::draw(&mut dl, fd, &layout, crate::HudMode::AirToGround);
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

/// Missionsende (M-032): Die Nachbesprechung ersetzt HUD und Brett –
/// Ausgang, Ereignisse seitenweise, Wertung; Absturz und Abzüge in Warnfarbe.
#[test]
fn debrief_pages_replace_hud() {
    use crate::debrief::{EVENTS_PER_SCREEN, Screen, page, screens};
    use f29_core::{CrashCause, Score, ScoreLine, Side, Status};
    let line =
        |k: u64| ScoreLine { time_s: k, text: "TAKEOFF".into(), points: if k == 0 { 500 } else { -30 } };
    let score = Score {
        lines: (0..EVENTS_PER_SCREEN as u64 + 1).map(line).collect(),
        subtotal: 500,
        factors: vec![("LANDED".into(), 1.0)],
        total: 500,
    };
    assert_eq!(
        screens(&score),
        [Screen::Outcome, Screen::Events(0), Screen::Events(EVENTS_PER_SCREEN), Screen::Rating]
    );
    let fd = |status: Status| f29_core::FlightData {
        status,
        mission_over: true,
        debrief: Some(score.clone()),
        ..sample()
    };
    let warns = |p: &crate::Page| p.rows.iter().any(|r| r.ink == Ink::HudWarning);
    let crashed = fd(Status::Crashed { tick: 1, cause: CrashCause::Terrain });
    assert!(
        build(&crashed, &ScreenLayout::cockpit(16.0 / 9.0), &HudOptions::default(), None).lines.is_empty()
    );
    assert!(warns(&page(&crashed, 0)), "Absturz in Warnfarbe");
    let landed = fd(Status::Landed { tick: 1, side: Side::Friendly });
    assert!(!warns(&page(&landed, 0)), "gelungene Landung nicht in Warnfarbe");
    assert!(warns(&page(&landed, 1)), "Abzüge in Warnfarbe");
    assert!(page(&landed, 3).rows.iter().any(|r| r.big && r.text == "TOTAL 500"));
    assert_eq!(page(&landed, 9), page(&landed, 3), "über das Ende hinaus der letzte");
}

#[test]
fn heading_formatting() {
    assert_eq!(heading_deg(0.0), 0);
    assert_eq!(heading_deg(std::f64::consts::TAU - 1e-6), 0);
    assert_eq!(heading_deg(std::f64::consts::FRAC_PI_2), 90);
}

/// Endanflug mit ausgefahrenem Fahrwerk: Landehilfen erscheinen, alles bleibt im Bild.
#[test]
fn landing_aids_and_ground_messages_stay_on_screen() {
    use f29_core::controls::flags;
    use f29_core::sim::Spawn;
    let mut sim = Sim::new(SimConfig::with(
        0x0F29,
        Spawn { altitude: 600.0, min_clearance: 300.0, speed: 80.0, throttle: 0.5, ..Spawn::default() },
    ));
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
    assert!(map.runways.len() >= 2 && map.route.len() >= 3);
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
        for zoom in 0..mfd::MAP_RANGES_KM.len() {
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
    // Gitterkarte zeichnet mehr Linien als die Satellitenansicht (Gitter)
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
    assert_eq!(s.map_zoom, mfd::MAP_RANGES_KM.len() - 1);
    // Reihum (Joystick-Taste): nach dem weitesten wieder der nächste.
    s.cycle_zoom();
    assert_eq!(s.map_zoom, 0);
}

/// Waffen in der Luft erscheinen auf der Karte: Rakete als Strich, Bombe als
/// Punkt, feindliche rot.
#[test]
fn munitions_show_on_the_map() {
    use f29_core::MunitionInfo;
    let sim = Sim::new(SimConfig::new(0x0F29));
    let base = sim.flight_data();
    let map = MapData::for_terrain(sim.terrain(), sim.route(), sim.targets());
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let m = layout.mfd_layout().unwrap();
    let ahead = base.position + f29_core::DVec3::new(base.heading.cos(), base.heading.sin(), 0.0) * 2_000.0;
    let count = |munitions: Vec<MunitionInfo>| {
        let mut fd = base.clone();
        fd.munitions = munitions;
        let opts = HudOptions { show_hud: false, ..HudOptions::default() };
        let dl = build(&fd, &layout, &opts, Some(&map));
        let left = |l: &&crate::Line| m.left.contains(l.a) && m.left.contains(l.b);
        (
            dl.lines.iter().filter(left).count(),
            dl.lines.iter().filter(left).filter(|l| l.ink == Ink::Warning).count(),
            dl.lines.iter().filter(left).filter(|l| l.ink == Ink::WarningOptical).count(),
        )
    };
    let w = MunitionInfo {
        position: ahead,
        velocity: f29_core::DVec3::new(500.0, 0.0, 0.0),
        missile: true,
        hostile: false,
        optical: false,
    };
    let (none, none_red, none_orange) = count(Vec::new());
    assert_eq!(count(vec![w]).0, none + 1, "Rakete als ein Strich");
    assert_eq!(count(vec![MunitionInfo { missile: false, ..w }]).0, none + 6, "Bombe als Punkt");
    assert_eq!(count(vec![MunitionInfo { hostile: true, ..w }]).1, none_red + 1, "Radarlenkung rot");
    let ir = count(vec![MunitionInfo { hostile: true, optical: true, ..w }]);
    assert_eq!((ir.1, ir.2), (none_red, none_orange + 1), "optisch/Infrarot orange");
    let own = count(vec![MunitionInfo { optical: true, ..w }]);
    assert_eq!(own.2, none_orange, "eigene Maverick bleibt in Flugzeugfarbe");
}

/// Feindliche Jäger erscheinen als roter Pfeil auf der Karte, mit Kreis,
/// wenn ihr Radar das Flugzeug sieht.
#[test]
fn fighters_show_on_the_map() {
    use f29_core::FighterInfo;
    let sim = Sim::new(SimConfig::new(0x0F29));
    let base = sim.flight_data();
    let map = MapData::for_terrain(sim.terrain(), sim.route(), sim.targets());
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let m = layout.mfd_layout().unwrap();
    let ahead = base.position + f29_core::DVec3::new(base.heading.cos(), base.heading.sin(), 0.0) * 3_000.0;
    let red = |fighters: Vec<FighterInfo>| {
        let mut fd = base.clone();
        fd.fighters = fighters;
        let opts = HudOptions { show_hud: false, ..HudOptions::default() };
        let dl = build(&fd, &layout, &opts, Some(&map));
        dl.lines.iter().filter(|l| m.left.contains(l.a) && l.ink == Ink::Warning).count()
    };
    let f = FighterInfo {
        id: 0,
        label: "MIG-29",
        position: ahead,
        velocity: f29_core::DVec3::ZERO,
        heading: 0.0,
        bank: 0.0,
        pursuing: false,
    };
    let none = red(Vec::new());
    assert_eq!(red(vec![f]), none + 2, "Pfeil aus zwei Strichen");
    assert_eq!(red(vec![FighterInfo { pursuing: true, ..f }]), none + 12, "mit Kreis");
}

/// Der gewählte Jäger: Ecken auf der Karte, Angaben auf der TGT-Seite im
/// AA-Modus (dort nicht im AG-Modus).
#[test]
fn picked_fighter_shows_on_map_and_target_page() {
    use crate::HudMode;
    let sim = Sim::new(SimConfig::new(0x0F29));
    let mut fd = sim.flight_data();
    fd.target = None;
    let map = MapData::for_terrain(sim.terrain(), sim.route(), sim.targets());
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let m = layout.mfd_layout().unwrap();
    let ahead = fd.position + f29_core::DVec3::new(fd.heading.cos(), fd.heading.sin(), 0.0) * 3_000.0;
    fd.fighters = vec![f29_core::FighterInfo {
        id: 3,
        label: "MIG-29",
        position: ahead,
        velocity: f29_core::DVec3::ZERO,
        heading: 0.0,
        bank: 0.0,
        pursuing: false,
    }];
    let mark = |selected, locked| f29_core::Mark {
        kind: f29_core::MarkKind::Air,
        position: ahead,
        direction_body: f29_core::DVec3::X,
        stage: f29_core::Stage::Detected,
        seen: f29_core::sensors::Seen { radar: false, optical: true },
        selected,
        locked,
    };
    fd.marks.push(mark(false, false));
    let count = |fd: &f29_core::FlightData, mode: HudMode| {
        let mfd = MfdState { right_page: RightPage::Target, ..MfdState::default() };
        let opts = HudOptions { show_hud: false, hud_mode: mode, mfd, ..HudOptions::default() };
        let dl = build(fd, &layout, &opts, Some(&map));
        let n = |r: Rect, ink: Option<Ink>| {
            dl.lines.iter().filter(|l| r.contains(l.a) && ink.is_none_or(|i| l.ink == i)).count()
        };
        (n(m.left, Some(Ink::Hud)), n(m.right_screen, None))
    };
    let mut picked = fd.clone();
    picked.air_target = Some(f29_core::PickedInfo {
        kind: f29_core::MarkKind::Air,
        index: 3,
        label: "MIG-29",
        side: f29_core::Side::Hostile,
        range: 3_000.0,
        closure_mps: 100.0,
        bearing: fd.heading,
        direction_body: f29_core::DVec3::X,
        seen: f29_core::sensors::Seen { radar: false, optical: true },
        locked: true,
    });
    *picked.marks.last_mut().unwrap() = mark(true, true);
    let (map0, page0) = count(&fd, HudMode::AirToAir);
    let (map1, page1) = count(&picked, HudMode::AirToAir);
    assert_eq!(map1, map0 + 16, "doppelter Rahmen und Ecken auf der Karte");
    let mut chosen = picked.clone();
    *chosen.marks.last_mut().unwrap() = mark(true, false);
    assert_eq!(count(&chosen, HudMode::AirToAir).0, map0 + 8, "gewählt, nicht erfasst: ohne Ecken");
    assert_eq!(
        count(&picked, HudMode::AirToGround).0,
        count(&fd, HudMode::AirToGround).0,
        "nur die Modusart"
    );
    assert!(page1 > page0 + 40, "Name und Angaben auf der TGT-Seite");
    assert_eq!(count(&picked, HudMode::AirToGround).1, count(&fd, HudMode::AirToGround).1);
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
            &HudOptions {
                show_hud: false,
                show_panel: true,
                mfd,
                camera_feed: false,
                ..HudOptions::default()
            },
            None,
        );
        dl.lines
            .iter()
            .filter(|l| {
                (l.ink == Ink::Needle || l.ink == Ink::HudWarning)
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
        hud::draw(&mut dl, fd, &layout, crate::HudMode::AirToGround);
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
        let opts = HudOptions {
            show_hud: false,
            show_panel: true,
            mfd: state,
            camera_feed: false,
            ..HudOptions::default()
        };
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

/// Offene Schachtklappen zeigt nur die Waffenanzeige unten links (blau, ohne
/// Blinken); die zentrale Warnung „BAY OPEN“ entfällt (v0.2.7).
#[test]
fn open_bay_shows_in_the_weapon_line() {
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let warn_lines = |door: f64, time_s: f64| {
        let mut fd = sample();
        fd.stores.door = door;
        fd.time_s = time_s;
        let dl = build(
            &fd,
            &layout,
            &HudOptions { show_panel: false, hud_mode: crate::HudMode::AirToGround, ..HudOptions::default() },
            None,
        );
        dl.lines.iter().filter(|l| l.ink == Ink::HudWarning).count()
    };
    let closed = warn_lines(0.0, 10.0);
    assert!(warn_lines(1.0, 10.0) > closed, "keine Warnung bei offenem Schacht");
    assert!(warn_lines(0.4, 10.0) > closed, "keine Warnung bei sich öffnendem Schacht");
    assert_eq!(warn_lines(1.0, 10.4), warn_lines(1.0, 10.0), "blinkt");
    assert_eq!(warn_lines(0.0, 10.4), closed);
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
    let Some(mut t) = base.target else { panic!("kein Ziel gewählt") };
    // Ziel genau voraus, damit der Rahmen im Glas liegt.
    t.direction_body = f29_core::DVec3::new(1.0, 0.0, 0.05).normalize_or_zero();
    t.civilian = false;
    let lines = |zone: Option<LaunchZone>| {
        let mut fd = base.clone();
        fd.target = Some(f29_core::TargetInfo { zone, ..t });
        let dl = build(
            &fd,
            &layout,
            &HudOptions { show_panel: false, hud_mode: crate::HudMode::AirToGround, ..HudOptions::default() },
            None,
        );
        (dl.lines.len(), dl.lines.iter().filter(|l| l.ink == Ink::HudWarning).count())
    };
    let (plain, plain_red) = lines(None);
    let (in_range, _) = lines(Some(LaunchZone::Locked));
    let (good, good_red) = lines(Some(LaunchZone::Good));
    let (none, _) = lines(Some(LaunchZone::NoEffect));
    assert_eq!(lines(Some(LaunchZone::NoLock)).0, plain);
    assert_eq!(in_range, plain + 8, "acht Eckstriche");
    assert!(good_red > plain_red + 8, "gute Zone blau");
    assert_eq!(good, in_range);
    assert_ne!(none, plain, "ohne Wirkung gestrichelt");
}

/// Bomben-Zielhilfe: Aufschlagpunkt mit Fall-Linie; zu tief rot.
#[test]
fn bomb_aim_draws_the_pipper() {
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let base = sample();
    let count = |aim: Option<f29_core::BombAim>| {
        let mut fd = base.clone();
        fd.roll = 0.0;
        fd.bomb_aim = aim;
        let dl = build(
            &fd,
            &layout,
            &HudOptions { show_panel: false, hud_mode: crate::HudMode::AirToGround, ..HudOptions::default() },
            None,
        );
        (dl.lines.len(), dl.lines.iter().filter(|l| l.ink == Ink::HudWarning).count())
    };
    let aim = f29_core::BombAim {
        impact: f29_core::DVec3::ZERO,
        direction_body: f29_core::DVec3::new(1.0, 0.0, 0.3).normalize_or_zero(),
        fall_time_s: 10.0,
        too_low: false,
        target_offset: None,
        lethal_radius_m: 30.0,
    };
    let (none, none_red) = count(None);
    let (pipper, _) = count(Some(aim));
    let (_, low_red) = count(Some(f29_core::BombAim { too_low: true, ..aim }));
    assert!(pipper > none, "kein Aufschlagpunkt");
    assert!(low_red > none_red, "zu tief nicht rot");
}

/// Ein Kreis je Modus (M-013): AG mit dem kleineren Bodenkreis, gleich
/// welche Waffe; AA mit dem größeren Luftkreis; NAV ohne.
#[test]
fn one_circle_per_mode() {
    use crate::HudMode;
    use f29_core::sensors::{AIR_CIRCLE_RAD, GROUND_CIRCLE_RAD};
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let fd = sample();
    let on_circle = |mode: HudMode, rad: f64| {
        let opts = HudOptions { show_panel: false, hud_mode: mode, ..HudOptions::default() };
        let dl = build(&fd, &layout, &opts, None);
        let (c, r) = (layout.view_rect().center(), crate::hud::circle_radius(&layout, rad));
        let on = |p: [f32; 2]| ((p[0] - c[0]).hypot(p[1] - c[1]) - r).abs() < 1e-4;
        dl.lines.iter().filter(|l| on(l.a) && on(l.b)).count()
    };
    assert_eq!(on_circle(HudMode::AirToGround, GROUND_CIRCLE_RAD), 40);
    assert_eq!(on_circle(HudMode::AirToAir, AIR_CIRCLE_RAD), 48);
    assert_eq!(on_circle(HudMode::Nav, GROUND_CIRCLE_RAD) + on_circle(HudMode::Nav, AIR_CIRCLE_RAD), 0);
}

/// Wie im Original zeigt das HUD nur das gewählte Ziel: Bekannte, nicht
/// gewählte Ziele bekommen in keinem Modus ein Viereck (die Karte zeigt sie).
#[test]
fn hud_shows_only_the_picked_target() {
    use crate::HudMode;
    use f29_core::{Mark, MarkKind, Stage};
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let mut fd = sample();
    let ahead = |y: f64| f29_core::DVec3::new(1.0, y, 0.02).normalize_or_zero();
    let mark = |kind, y, stage, radar| Mark {
        kind,
        position: f29_core::DVec3::ZERO,
        direction_body: ahead(y),
        stage,
        seen: f29_core::sensors::Seen { radar, optical: false },
        selected: false,
        locked: false,
    };
    fd.marks = vec![
        mark(MarkKind::Ground, -0.1, Stage::Reported, false),
        mark(MarkKind::Air, 0.0, Stage::Reported, false),
        mark(MarkKind::Air, 0.05, Stage::Detected, true),
        mark(MarkKind::Runway, 0.1, Stage::Detected, false),
    ];
    let dim = |fd: &f29_core::FlightData, mode: HudMode| {
        let opts = HudOptions { show_panel: false, hud_mode: mode, ..HudOptions::default() };
        build(fd, &layout, &opts, None).lines.iter().filter(|l| l.ink == Ink::HudDim).count()
    };
    let empty = f29_core::FlightData { marks: Vec::new(), ..fd.clone() };
    let added = |mode| dim(&fd, mode) - dim(&empty, mode);
    for mode in [HudMode::Nav, HudMode::AirToGround, HudMode::AirToAir] {
        assert_eq!(added(mode), 0, "{mode:?}");
    }
}

/// Feuersignal: „SHOOT“ (blau), wenn die Vorausberechnung trifft, sonst nur
/// „LOCKED“.
#[test]
fn shoot_cue_follows_the_zone() {
    use f29_core::weapons::{Bay, LaunchZone, WeaponKind};
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let mut sim = Sim::new(SimConfig::new(0x0F29));
    sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.55, f29_core::controls::flags::RADAR_TOGGLE));
    let base = sim.flight_data();
    let t0 = base.target.expect("Ziel");
    let red = |kind: WeaponKind, zone: LaunchZone, off_deg: f64| {
        let mut fd = base.clone();
        fd.stores.selected = 0;
        fd.stores.gun_selected = false;
        fd.stores.bays[0] = Bay { kind: Some(kind), count: 1 };
        fd.bomb_aim = None;
        let a = off_deg.to_radians();
        fd.target = Some(f29_core::TargetInfo {
            zone: Some(zone),
            civilian: false,
            direction_body: f29_core::DVec3::new(a.cos(), 0.0, a.sin()),
            ..t0
        });
        let dl = build(
            &fd,
            &layout,
            &HudOptions { show_panel: false, hud_mode: crate::HudMode::AirToGround, ..HudOptions::default() },
            None,
        );
        dl.lines.iter().filter(|l| l.ink == Ink::HudWarning).count()
    };
    let lgb_good = red(WeaponKind::GuidedBomb, LaunchZone::Good, 25.0);
    let lgb_in = red(WeaponKind::GuidedBomb, LaunchZone::Locked, 25.0);
    assert!(lgb_good > lgb_in + 10, "kein SHOOT für die Laserbombe");
    let agm_good = red(WeaponKind::AirToGround, LaunchZone::Good, 4.0);
    let agm_out = red(WeaponKind::AirToGround, LaunchZone::NoLock, 4.0);
    assert!(agm_good > agm_out + 10, "kein SHOOT für die Rakete");
}

/// Karte: Ziele außerhalb der eigenen Erfassung nur mit AWACS (Zieltyp
/// gedämpft); ohne AWACS fehlen sie (außer Primär-/Sekundärziel).
#[test]
fn awacs_shows_targets_beyond_own_sensors() {
    let sim = Sim::new(SimConfig::new(0x0F29));
    let map = MapData::for_terrain(sim.terrain(), sim.route(), sim.targets());
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let mut fd = sim.flight_data();
    fd.own_contacts.clear();
    let mut opts = HudOptions { show_hud: false, ..HudOptions::default() };
    opts.mfd.map_zoom = 7;
    let lines = |awacs: bool| {
        let mut f = fd.clone();
        f.awacs = awacs;
        build(&f, &layout, &opts, Some(&map)).lines.len()
    };
    assert!(lines(true) > lines(false) + 20, "AWACS-Daten fehlen auf der Karte");
}

/// Vorhaltemarke (Kreis mit Punkt) nur im AA-Modus, mit gewählter Kanone
/// und Munition.
#[test]
fn lead_marker_shows_in_air_to_air() {
    use crate::HudMode;
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let mut fd = sample();
    fd.stores.gun_selected = true;
    fd.gun.rounds = 100;
    let count = |fd: &f29_core::FlightData, mode: HudMode| {
        let opts = HudOptions { show_panel: false, hud_mode: mode, ..HudOptions::default() };
        build(fd, &layout, &opts, None).lines.len()
    };
    let mut led = fd.clone();
    led.gun.lead = Some(f29_core::DVec3::new(1.0, 0.03, -0.02).normalize_or_zero());
    assert_eq!(count(&led, HudMode::AirToAir), count(&fd, HudMode::AirToAir) + 22);
    assert_eq!(count(&led, HudMode::AirToGround), count(&fd, HudMode::AirToGround));
    led.stores.gun_selected = false;
    let mut plain = led.clone();
    plain.gun.lead = None;
    assert_eq!(count(&led, HudMode::AirToAir), count(&plain, HudMode::AirToAir), "ohne Kanone keine Marke");
}

/// Kanonenkreuz am Aufschlagpunkt (vier Striche), nur mit Munition; der
/// Munitionsstand steht unten links.
#[test]
fn gun_cross_marks_the_impact_point() {
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let base = sample();
    let count = |aim: Option<f29_core::GunAim>, rounds: u16| {
        let mut fd = base.clone();
        fd.gun = f29_core::GunInfo { rounds, firing: false, aim, lead: None };
        build(
            &fd,
            &layout,
            &HudOptions { show_panel: false, hud_mode: crate::HudMode::AirToGround, ..HudOptions::default() },
            None,
        )
        .lines
        .len()
    };
    let aim = f29_core::GunAim {
        impact: f29_core::DVec3::ZERO,
        direction_body: f29_core::DVec3::new(1.0, 0.0, 0.08).normalize_or_zero(),
        range_m: 1_500.0,
    };
    assert_eq!(count(Some(aim), 600), count(None, 600) + 4);
    // Leer: kein Kreuz mehr.
    assert_eq!(count(Some(aim), 0), count(None, 0));
}

/// HUD-Modi: NAV ohne Waffensymbolik, AG mit Zielrahmen und Waffe, AA mit
/// großem Boresight-Kreis; die Landehilfen nur in NAV.
#[test]
fn hud_modes_show_their_symbology() {
    use crate::HudMode;
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let mut sim = Sim::new(SimConfig::new(0x0F29));
    sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.55, f29_core::controls::flags::RADAR_TOGGLE));
    let mut fd = sim.flight_data();
    let mut t = fd.target.expect("Ziel");
    t.direction_body = f29_core::DVec3::new(1.0, 0.0, 0.05).normalize_or_zero();
    fd.target = Some(t);
    let draw = |fd: &f29_core::FlightData, mode: HudMode| {
        build(fd, &layout, &HudOptions { show_panel: false, hud_mode: mode, ..HudOptions::default() }, None)
            .lines
    };
    let c = layout.view_rect().center();
    let r = crate::hud::circle_radius(&layout, f29_core::sensors::AIR_CIRCLE_RAD);
    let on_aa = |l: &crate::Line| ((l.a[0] - c[0]).hypot(l.a[1] - c[1]) - r).abs() < 1e-4;
    assert_eq!(draw(&fd, HudMode::AirToAir).iter().filter(|l| on_aa(l)).count(), 48, "AA-Kreis");
    assert_eq!(draw(&fd, HudMode::Nav).iter().filter(|l| on_aa(l)).count(), 0);
    // Zielrahmen und Waffenzeile nur in den Waffenmodi.
    assert!(draw(&fd, HudMode::AirToGround).len() > draw(&fd, HudMode::Nav).len() + 20);
    // Landemodus: Gleitlinie und Sinkratenskala nur in NAV.
    let mut land = fd.clone();
    land.gear = 1.0;
    land.on_ground = false;
    let gained = |mode: HudMode| draw(&land, mode).len() as i64 - draw(&fd, mode).len() as i64;
    assert!(gained(HudMode::Nav) > gained(HudMode::AirToGround) + 5);
    // Erfasster Jäger: Rahmen nur im AA-Modus.
    let mut air = fd.clone();
    air.air_target = Some(f29_core::PickedInfo {
        kind: f29_core::MarkKind::Air,
        index: 0,
        label: "MIG-21",
        side: f29_core::Side::Hostile,
        range: 4_000.0,
        closure_mps: 150.0,
        bearing: 0.0,
        direction_body: f29_core::DVec3::new(1.0, 0.05, -0.02).normalize_or_zero(),
        seen: f29_core::sensors::Seen { radar: false, optical: true },
        locked: true,
    });
    let blue = |fd: &f29_core::FlightData, mode: HudMode| {
        draw(fd, mode).iter().filter(|l| l.ink == Ink::HudWarning).count() as i64
    };
    assert!(blue(&air, HudMode::AirToAir) >= blue(&fd, HudMode::AirToAir) + 12, "Rahmen mit Ecken fehlt");
    assert_eq!(blue(&air, HudMode::AirToGround), blue(&fd, HudMode::AirToGround));
    // Reihenfolge der Modi
    assert_eq!(HudMode::default(), HudMode::Nav);
    assert_eq!(HudMode::Nav.next().next().next(), HudMode::Nav);
}

/// Stealth-Anzeige: ein Balken für das Stealth-Level; ein Ping erscheint als
/// Strich, rot, wenn er darüber liegt (Treffer); Alarmstufen als Lampen. Entdeckende Stellungen zeigen auf
/// der Karte ihren Reichweitenkreis.
#[test]
fn signature_gauge_and_radar_circles() {
    use f29_core::radar::RadarKind;
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let m = layout.mfd_layout().unwrap();
    let sim = Sim::new(SimConfig::new(0x0F29));
    let map = MapData::for_terrain(sim.terrain(), sim.route(), sim.targets());
    let base = sim.flight_data();
    let site = |signal: f64, alert: u8| f29_core::ThreatSite {
        site: 0,
        target: 0,
        name: "SAM SITE",
        kind: RadarKind::Doppler,
        signal,
        alert,
        range_m: 10_000.0,
    };
    let red_in = |fd: &f29_core::FlightData, r: crate::Rect| {
        let opts = HudOptions { show_hud: false, ..HudOptions::default() };
        build(fd, &layout, &opts, Some(&map))
            .lines
            .iter()
            .filter(|l| l.ink == Ink::Warning && r.contains(l.a) && r.contains(l.b))
            .count()
    };
    let mut hidden = base.clone();
    hidden.threat.stealth = 0.6;
    hidden.threat.worst = Some(site(0.4, 0));
    let ping = |signal: f64, age_s: f64| {
        let mut fd = hidden.clone();
        fd.threat.last_ping = Some(f29_core::PingInfo {
            tick: 1,
            age_s,
            signal,
            stealth: 0.6,
            hit: signal > 0.6,
            kind: RadarKind::Doppler,
        });
        fd
    };
    assert_eq!(red_in(&hidden, m.center), 0);
    // Ein Ping über dem Stealth-Level: roter Strich; darunter nicht rot;
    // nach einer Sekunde verschwindet er.
    assert!(red_in(&ping(0.8, 0.2), m.center) >= 4, "Treffer-Ping nicht rot");
    assert_eq!(red_in(&ping(0.4, 0.2), m.center), 0);
    assert_eq!(red_in(&ping(0.8, 1.5), m.center), 0);
    let mut alarmed = hidden.clone();
    alarmed.threat.max_alert = 3;
    assert!(red_in(&alarmed, m.center) >= 12, "Alarmlampen");
    // Karte: Kreis um eine Stellung mit Alarmstufe
    let mut on_map = hidden.clone();
    let me = sim.aircraft().position;
    on_map.threat.sites = vec![f29_core::RadarSiteInfo {
        target: 0,
        pos: [me.x + 2_000.0, me.y],
        range_m: 4_000.0,
        kind: RadarKind::Doppler,
        alert: 3,
    }];
    assert!(red_in(&on_map, m.left_screen) > red_in(&hidden, m.left_screen) + 10, "kein Reichweitenkreis");
}

/// Weitwinkelsicht: kein Brett, keine MFDs, gleicher Maßstab der Ansicht.
#[test]
fn wide_view_drops_the_panel_at_the_same_scale() {
    let (cockpit, wide) = (ScreenLayout::cockpit(16.0 / 9.0), ScreenLayout::wide(16.0 / 9.0));
    assert!((wide.focal() - cockpit.focal()).abs() < 1e-5, "Maßstab");
    assert!(wide.vertical_fov > cockpit.vertical_fov);
    assert!(wide.mfd_layout().is_none());
    let fd = sample();
    let lines = |l: &ScreenLayout| build(&fd, l, &HudOptions::default(), None).lines.len();
    assert!(lines(&wide) < lines(&cockpit), "Brett gezeichnet");
}

/// Ausgestiegen: kein HUD (den Schirm zeigt die 3D-Ansicht).
#[test]
fn ejected_pilot_sees_no_hud() {
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let opts = HudOptions { show_panel: false, ..HudOptions::default() };
    let pilot = f29_core::PilotInfo {
        position: f29_core::DVec3::ZERO,
        heading: 0.0,
        seconds: 3.0,
        chute: 1.0,
        speed: 6.0,
    };
    let fd = f29_core::FlightData { pilot: Some(pilot), ..sample() };
    assert!(build(&fd, &layout, &opts, None).lines.is_empty());
}

/// Bewaffnung (M-036): Der ganze Umriss liegt im Bildfeld, die Schächte im
/// Umriss; der Schacht im Fokus ist doppelt gerahmt.
#[test]
fn arming_fits_the_picture() {
    use crate::arming::{Arming, bay_rect, draw};
    let area = Rect { min: [-0.3, -0.2], max: [0.7, 0.3] };
    let inside =
        |p: [f32; 2], r: Rect| (r.min[0]..=r.max[0]).contains(&p[0]) && (r.min[1]..=r.max[1]).contains(&p[1]);
    let plain = draw(&Arming::default(), area).lines;
    assert!(plain.iter().all(|l| inside(l.a, area) && inside(l.b, area)), "Umriss ragt hinaus");
    for k in 0..4 {
        let r = bay_rect(k, area);
        assert!(inside(r.min, area) && inside(r.max, area));
    }
    assert!(draw(&Arming { bay: Some(2), ..Arming::default() }, area).lines.len() > plain.len());
}

/// Lagebesprechung (M-036): Die Bedrohungen zeichnen die Radarkreise dazu,
/// der Flugplan beschriftet die Wegpunkte, die Planquadrate tragen ihre
/// Buchstaben (M-031); alles bleibt im Bildfeld.
#[test]
fn intel_map_shows_threats_and_plan() {
    use crate::intel::{MapView, Topic, draw, labels};
    let sim = Sim::new(SimConfig::runway(7));
    let map = crate::MapData::for_terrain(sim.terrain(), sim.route(), sim.targets());
    let view = MapView::fit(&map, f29_core::nav::HALF_EXTENT_M);
    let fd = sim.flight_data();
    let area = Rect { min: [0.2, -0.25], max: [0.75, 0.3] };
    let lines = |t| draw(&map, &fd.threat.sites, t, view, area).lines;
    assert!(lines(Topic::Threats).len() > lines(Topic::Leave).len() + 64);
    let (squares, plan): (Vec<_>, Vec<_>) =
        labels(&map, Topic::FlightPlan, view, area).into_iter().partition(|l| l.ink == Ink::MfdDim);
    assert!(!squares.is_empty() && squares.iter().all(|l| l.text.len() == 2 && area.contains(l.pos)));
    let texts: Vec<&str> = plan.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(texts, ["HOME", "2", "3"]);
    let inside = |p: [f32; 2]| {
        (area.min[0] - 0.1..area.max[0] + 0.1).contains(&p[0]) && area.contains([area.center()[0], p[1]])
    };
    assert!(plan.iter().all(|l| inside(l.pos)), "{plan:?}");
}

/// Lagebesprechung: Zoomen hält den Punkt unter dem Zeiger fest, Ziehen
/// verschiebt die Karte mit dem Zeiger; der Ausschnitt bleibt im Gebiet
/// und zwischen engstem Zoom und ganzem Gebiet.
#[test]
fn intel_map_zooms_and_drags_within_the_theater() {
    use crate::intel::{MIN_HALF_M, MapView};
    let area = Rect { min: [0.0, 0.0], max: [0.5, 0.5] };
    let bound = 250_000.0;
    let v = MapView { center: [10_000.0, -20_000.0], half: 80_000.0 };
    let at = [0.4, 0.1];
    let z = v.zoom(area, at, 0.5, bound);
    assert!((z.half - 40_000.0).abs() < 1e-6);
    let (a, b) = (v.world(area, at), z.world(area, at));
    assert!((a[0] - b[0]).abs() < 1.0 && (a[1] - b[1]).abs() < 1.0, "{a:?} {b:?}");
    assert_eq!(v.zoom(area, at, 1e-3, bound).half, MIN_HALF_M);
    assert_eq!(v.zoom(area, at, 1e3, bound), MapView { center: [0.0, 0.0], half: bound });
    // Zeiger nach rechts oben: Die Mitte rückt nach Südwest.
    let d = v.drag(area, [0.05, 0.05], bound);
    assert!(d.center[0] < v.center[0] && d.center[1] < v.center[1]);
    let far = v.drag(area, [-100.0, 0.0], bound);
    assert_eq!(far.center[1], bound - v.half, "am Gebietsrand");
}

/// Ehrung: die Ereignisse des Einsatzes, darunter alle Auszeichnungen,
/// höchste zuerst; selbst eine volle Akte passt ins Textfeld (15 Zeilen,
/// `f29-app/src/ui.rs`).
#[test]
fn honors_list_events_and_decorations() {
    use f29_core::roster::{MEDALS, Pilot, RIBBONS, Report, Settings, Status};
    let mut p = Pilot::new("MAVERICK", Settings::default());
    (p.rank, p.missions, p.medals, p.purple_heart, p.status) =
        (3, 50, [9, 2, 1, 3, 1], true, Status::Retired);
    let r = Report { promoted: true, delayed: false, medal: Some(2), purple_heart: true, ribbon: Some(3) };
    let page = crate::debrief::honors(&p, &r);
    assert_eq!(page.title, "MAJOR MAVERICK");
    let shows = |t: &str| page.rows.iter().any(|row| row.text == t || row.value == t);
    assert!(shows("SILVER STAR, PURPLE HEART") && shows(RIBBONS[3].1) && shows("RETIRED FROM ACTIVE DUTY"));
    assert!(page.rows.len() <= 15, "{} Zeilen", page.rows.len());
    let first = page.rows.iter().position(|r| r.text == "DECORATIONS").unwrap() + 1;
    assert_eq!(page.rows[first].text, MEDALS[4].0, "höchste zuerst");
}
