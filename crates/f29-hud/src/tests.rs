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
        let dl = build(&fd, &layout, &HudOptions::default());
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
    let a = build(&fd, &layout, &HudOptions::default());
    let b = build(&fd, &layout, &HudOptions::default());
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
    let dl = build(&fd, &layout, &HudOptions { show_hud: true, show_panel: false });
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
    let with = build(&fd, &layout, &HudOptions::default());
    let mut clean = fd.clone();
    clean.gear = 0.0;
    let without = build(&clean, &layout, &HudOptions::default());
    assert!(with.lines.len() > without.lines.len() + 8, "keine Landehilfen");

    let mut ground = Sim::new(SimConfig::runway(0x0F29));
    ground.step(ControlFrame::from_analog(0.0, 0.0, 0.0, 0.0, flags::GEAR));
    for aspect in [4.0 / 3.0, 16.0 / 9.0, 21.0 / 9.0] {
        let layout = ScreenLayout::cockpit(aspect);
        for fd in [fd.clone(), ground.flight_data()] {
            let dl = build(&fd, &layout, &HudOptions::default());
            let (min, max) = bounds(&dl);
            assert!(min[0] >= -aspect / 2.0 - 1e-4 && max[0] <= aspect / 2.0 + 1e-4);
            assert!(min[1] >= -0.5 - 1e-4 && max[1] <= 0.5 + 1e-4);
        }
    }
}
