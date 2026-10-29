//! Verhaltenstests über die Kennwerte aus `f29_core::envelope`: jeder Wert
//! gegen ein Toleranzband statt einzelner, chaotischer Flugbahnen.
//! Übersicht der aktuellen Werte: `cargo run --release -p f29-core --example flight_report`.

use f29_core::envelope::{self, Metric};
use std::sync::LazyLock;

static REPORT: LazyLock<Vec<Metric>> = LazyLock::new(envelope::report);

fn v(id: &str) -> f64 {
    envelope::get(&REPORT, id)
}

fn within(id: &str, lo: f64, hi: f64) {
    let x = v(id);
    assert!((lo..=hi).contains(&x), "{id} = {x:.1} liegt nicht in {lo}..{hi}");
}

#[test]
fn stall_speeds() {
    within("vs_clean", 115.0, 135.0);
    let ratio = v("vs_flaps") / v("vs_clean");
    assert!((0.8..0.92).contains(&ratio), "Klappen senken die Abrissfahrt auf {ratio:.2}");
}

/// Langsam mit gehaltener Nase sackt das Flugzeug durch, Klappen lindern das,
/// schnell gibt es kein Durchsacken (M-004, v0.1.15).
#[test]
fn slow_flight_sinks_and_flaps_relieve_it() {
    assert!(v("sink_clean") < -200.0, "kaum Durchsacken: {:.0} ft", v("sink_clean"));
    assert!(v("sink_flaps") > v("sink_clean") + 150.0, "Klappen lindern kaum");
    assert!(v("sink_fast") > -50.0, "schnell sackt es durch: {:.0} ft", v("sink_fast"));
}

/// Ein Abriss kostet viel Höhe, mit Klappen weniger, aber deutlich (M-004, v0.1.16).
#[test]
fn stalls_cost_a_lot_of_height() {
    within("stall_level_clean", 2_500.0, 8_000.0);
    let flaps = v("stall_level_flaps") / v("stall_level_clean");
    assert!((0.3..0.85).contains(&flaps), "mit Klappen {flaps:.2} des Verlusts ohne");
    within("stall_zoom_clean", 3_000.0, 8_000.0);
    assert!(v("stall_zoom_flaps") < v("stall_zoom_clean"), "Klappen helfen im Steigflugabriss nicht");
    // Kurzer, beschleunigter Abriss bei hoher Fahrt: schnell vorbei.
    within("stall_turn_clean", 0.0, 1_000.0);
}

/// Harte Kurven reißen schnell ab, Hochziehen erst, wenn die Fahrt verbraucht ist.
#[test]
fn hard_turns_stall_sooner_than_level_pulls() {
    within("stall_time_turn", 0.0, 2.5);
    assert!(v("stall_time_level") > v("stall_time_turn") + 2.0, "Hochziehen reißt zu früh ab");
}

/// Harte Kurven kosten Fahrt, Manöverklappen weniger.
#[test]
fn hard_turns_bleed_energy_less_with_flaps() {
    within("turn_speed_clean", 300.0, 420.0);
    assert!(v("turn_speed_flaps") > v("turn_speed_clean") + 5.0, "Klappen sparen in der Kurve keine Fahrt");
}

/// Voll gezogener Start: kein Abriss direkt über der Bahn (M-004, v0.1.9).
#[test]
fn full_pull_takeoff() {
    assert_eq!(v("takeoff_release_stall"), 0.0, "Abriss trotz Loslassen bei 15°");
    assert!(v("takeoff_release_agl") > 800.0, "nach dem Loslassen kaum gestiegen");
    assert!(
        v("takeoff_hold_stall_agl") > 200.0,
        "Abriss schon {:.0} ft über Grund",
        v("takeoff_hold_stall_agl")
    );
    assert!(v("takeoff_hold_flaps_max_agl") > 490.0, "mit Klappen kein kräftiger Steigflug");
}

#[test]
fn autopilot_lands_softly() {
    assert_eq!(v("landing_ok"), 1.0, "Lande-Autopilot scheitert");
    within("landing_sink", 0.0, 600.0);
}
