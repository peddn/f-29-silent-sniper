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

#[test]
fn crash_screen_replaces_hud() {
    let mut fd = sample();
    fd.warnings.crashed = true;
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let dl = build(
        &fd,
        &layout,
        &HudOptions { show_panel: false, hud_mode: crate::HudMode::AirToGround, ..HudOptions::default() },
        None,
    );
    assert!(dl.lines.iter().all(|l| l.ink == Ink::HudWarning));
}

#[test]
fn heading_formatting() {
    assert_eq!(heading_deg(0.0), 0);
    assert_eq!(heading_deg(std::f64::consts::TAU - 1e-6), 0);
    assert_eq!(heading_deg(std::f64::consts::FRAC_PI_2), 90);
}

#[test]
fn sink_marks_fit_the_scale() {
    // Beide Marken liegen auf der 10-m/s-Skala, hart unter Bruch.
    const {
        assert!(crate::hud::HARD_SINK_MPS < crate::hud::MAX_SINK_MPS);
        assert!(crate::hud::MAX_SINK_MPS < 10.0);
    };
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
        )
    };
    let w = MunitionInfo {
        position: ahead,
        velocity: f29_core::DVec3::new(500.0, 0.0, 0.0),
        missile: true,
        hostile: false,
    };
    let (none, none_red) = count(Vec::new());
    assert_eq!(count(vec![w]).0, none + 1, "Rakete als ein Strich");
    assert_eq!(count(vec![MunitionInfo { missile: false, ..w }]).0, none + 6, "Bombe als Punkt");
    assert_eq!(count(vec![MunitionInfo { hostile: true, ..w }]).1, none_red + 1, "feindlich rot");
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
    let Some(mut t) = base.target else { panic!("kein Ziel aufgeschaltet") };
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
    let (in_range, _) = lines(Some(LaunchZone::InRange));
    let (good, good_red) = lines(Some(LaunchZone::Good));
    let (none, _) = lines(Some(LaunchZone::NoEffect));
    assert_eq!(lines(Some(LaunchZone::OutOfRange)).0, plain);
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

/// Abschusskreis nur bei gewählter Rakete (nicht bei Bomben oder Laserbombe).
#[test]
fn launch_circle_only_for_missiles() {
    use f29_core::weapons::{Bay, WeaponKind};
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let base = sample();
    let lines = |kind: WeaponKind| {
        let mut fd = base.clone();
        fd.stores.selected = 0;
        fd.stores.gun_selected = false;
        fd.stores.bays[0] = Bay { kind: Some(kind), count: 1 };
        fd.bomb_aim = None;
        // Nur Linien auf dem Kreisradius um die Glasmitte zählen.
        let dl = build(
            &fd,
            &layout,
            &HudOptions { show_panel: false, hud_mode: crate::HudMode::AirToGround, ..HudOptions::default() },
            None,
        );
        let c = layout.view_rect().center();
        let r = layout.focal() * (f29_core::weapons::LAUNCH_CIRCLE_RAD as f32).tan();
        let on = |p: [f32; 2]| ((p[0] - c[0]).hypot(p[1] - c[1]) - r).abs() < 1e-4;
        dl.lines.iter().filter(|l| on(l.a) && on(l.b)).count()
    };
    let bomb = lines(WeaponKind::FreeFall);
    assert_eq!(lines(WeaponKind::GuidedBomb), bomb);
    assert!(lines(WeaponKind::AirToGround) >= bomb + 30, "kein Kreis bei der Maverick");
    assert_eq!(lines(WeaponKind::AntiRadar), lines(WeaponKind::AirToGround));
}

/// Feuersignal: „SHOOT“ im Sweet Spot (rot), „IN RANGE“ sonst; bei Raketen
/// nur „SHOOT“, wenn das Ziel im Abschusskreis liegt.
#[test]
fn shoot_cue_follows_zone_and_circle() {
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
    let lgb_in = red(WeaponKind::GuidedBomb, LaunchZone::InRange, 25.0);
    assert!(lgb_good > lgb_in + 10, "kein SHOOT für die Laserbombe");
    // Rakete: Ziel 4° unter der Nase (im Kreis) vs. 15° (außerhalb).
    let in_circle = red(WeaponKind::AirToGround, LaunchZone::Good, 4.0);
    let outside = red(WeaponKind::AirToGround, LaunchZone::Good, 15.0);
    assert!(in_circle > outside + 10, "SHOOT trotz Ziel außerhalb des Kreises");
}

/// Ecken (= Rakete würde übernehmen) nur, wenn das Ziel im Abschusskreis liegt.
#[test]
fn corners_only_inside_the_launch_circle() {
    use f29_core::weapons::{Bay, LaunchZone, WeaponKind};
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let mut sim = Sim::new(SimConfig::new(0x0F29));
    sim.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.55, f29_core::controls::flags::RADAR_TOGGLE));
    let base = sim.flight_data();
    let t0 = base.target.expect("Ziel");
    let lines = |off_deg: f64, zone: Option<LaunchZone>| {
        let mut fd = base.clone();
        fd.stores.selected = 0;
        fd.stores.gun_selected = false;
        fd.stores.bays[0] = Bay { kind: Some(WeaponKind::AirToGround), count: 1 };
        fd.bomb_aim = None;
        let a = off_deg.to_radians();
        fd.target = Some(f29_core::TargetInfo {
            zone,
            civilian: false,
            direction_body: f29_core::DVec3::new(a.cos(), a.sin(), 0.0),
            ..t0
        });
        build(
            &fd,
            &layout,
            &HudOptions { show_panel: false, hud_mode: crate::HudMode::AirToGround, ..HudOptions::default() },
            None,
        )
        .lines
        .len()
    };
    // 12° neben der Nase: außerhalb des Kreises, aber noch im Glas.
    assert_eq!(lines(12.0, Some(LaunchZone::InRange)), lines(12.0, Some(LaunchZone::OutOfRange)));
    assert!(lines(4.0, Some(LaunchZone::InRange)) >= lines(4.0, Some(LaunchZone::OutOfRange)) + 8);
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
    opts.mfd.map_zoom = 4;
    let lines = |awacs: bool| {
        let mut f = fd.clone();
        f.awacs = awacs;
        build(&f, &layout, &opts, Some(&map)).lines.len()
    };
    assert!(lines(true) > lines(false) + 20, "AWACS-Daten fehlen auf der Karte");
}

/// Kanonenkreuz am Aufschlagpunkt (vier Striche), nur mit Munition; der
/// Munitionsstand steht unten links.
#[test]
fn gun_cross_marks_the_impact_point() {
    let layout = ScreenLayout::cockpit(16.0 / 9.0);
    let base = sample();
    let count = |aim: Option<f29_core::GunAim>, rounds: u16| {
        let mut fd = base.clone();
        fd.gun = f29_core::GunInfo { rounds, firing: false, aim };
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
    let r = layout.focal() * crate::hud::AA_CIRCLE_DEG.to_radians().tan();
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
    let t = &sim.targets()[0];
    on_map.threat.sites = vec![f29_core::RadarSiteInfo {
        target: 0,
        pos: [t.north, t.east],
        range_m: 30_000.0,
        kind: RadarKind::Doppler,
        alert: 3,
    }];
    assert!(red_in(&on_map, m.left_screen) > red_in(&hidden, m.left_screen) + 10, "kein Reichweitenkreis");
}
