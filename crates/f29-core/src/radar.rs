//! Feindliche Radarstellungen und Entdeckung (M-020).
//!
//! Jede Radarstellung „pingt“ einmal je Antennenumlauf. Bei diesem Ping
//! trifft ihr Impuls mit einer bestimmten Stärke beim Flugzeug ein; ist er
//! stärker als das Stealth-Level des Flugzeugs gegenüber dieser Stellung, ist
//! es ein Treffer und die Alarmstufe der Stellung steigt um eins (höchstens 3):
//!
//! 1. **Entdeckt** – die Stellung kennt das Flugzeug („RADAR DETECTED“).
//! 2. **Verfolgt** – sie merkt sich den Ort des Pings (Ziel späterer
//!    Patrouillen).
//! 3. **Radar ID** – Auslöser für den Raketenstart (kommt mit dem
//!    Schadensmodell).
//!
//! Ohne Treffer sinkt die Stufe nach [`DECAY_S`] um eins. Radare sehen durch
//! das Gelände (keine Sichtlinie, Spielerentscheidung v0.2.8).
//!
//! Das Stealth-Level ist wie im Original ein Wert für alle Radare: tief und
//! waagerecht ist am unauffälligsten; Höhe über Grund, Querlage, offener
//! Schacht und eigenes Radar kosten. Am Boden ist das Flugzeug unsichtbar.
//!
//! Zwei Radartypen unterscheiden sich im Impuls:
//! - **Impulsradar** (Frühwarnung, große Reichweite): richtungsunabhängig.
//! - **Dopplerradar** (SAM-Stellung): sieht Bewegung auf sich zu oder von sich
//!   weg; wer quer zu ihm fliegt, ist schwer zu sehen.

use crate::TICK_HZ;
use crate::detmath;
use crate::math::DVec3;
use crate::targets::{Target, TargetClass};

/// Ohne Treffer sinkt die Alarmstufe nach so vielen Sekunden um eins.
pub const DECAY_S: f64 = 30.0;
/// Höchste Alarmstufe.
pub const MAX_ALERT: u8 = 3;
/// Einträge im Ereignisprotokoll.
const EVENT_LOG: usize = 16;

/// Art des Radars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadarKind {
    /// Impulsradar: richtungsunabhängig, im Tiefflug schwach (Bodenechos).
    Pulse,
    /// Dopplerradar: im Querflug schwach.
    Doppler,
}

impl RadarKind {
    pub fn label(self) -> &'static str {
        match self {
            RadarKind::Pulse => "PULSE",
            RadarKind::Doppler => "DOPPLER",
        }
    }
}

/// Kenndaten einer Radarstellung.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadarSpec {
    pub kind: RadarKind,
    /// Reichweite (m): an der Grenze ist der Impuls null.
    pub range_m: f64,
    /// Antennenumlauf (s): so oft pingt die Stellung.
    pub sweep_s: f64,
}

/// Frühwarnradar: weit, langsamer Umlauf (eigene Werte).
pub const EARLY_WARNING: RadarSpec = RadarSpec { kind: RadarKind::Pulse, range_m: 60_000.0, sweep_s: 10.0 };
/// SAM-Feuerleitradar: kürzer, schneller Umlauf (eigene Werte).
pub const SAM: RadarSpec = RadarSpec { kind: RadarKind::Doppler, range_m: 30_000.0, sweep_s: 5.0 };

/// Radar einer Zielklasse, falls sie eines hat.
pub fn spec_for(class: TargetClass) -> Option<RadarSpec> {
    match class {
        TargetClass::EarlyWarning => Some(EARLY_WARNING),
        TargetClass::AirDefense => Some(SAM),
        _ => None,
    }
}

/// Dopplerradar im reinen Querflug.
const BEAM_FLOOR: f64 = 0.25;

/// Sichtbarkeit, die immer bleibt (tief, waagerecht, alles zu).
pub const SIG_BASE: f64 = 0.10;
/// Höhe über Grund: bis hier am unauffälligsten (m), darüber steigt die
/// Sichtbarkeit wie im Original zunächst schnell, dann immer langsamer
/// (Arkustangens) bis höchstens [`SIG_ALTITUDE`].
pub const LOW_LEVEL_M: f64 = 100.0;
const ALTITUDE_SCALE_M: f64 = 1_500.0;
pub const SIG_ALTITUDE: f64 = 0.40;
/// Querlage: linear bis 90° (dann zeigt das Flugzeug seine ganze Draufsicht);
/// darüber wieder fallend (Rückenflug wie waagerecht).
pub const SIG_BANK: f64 = 0.30;
/// Offener Schacht.
pub const SIG_BAY: f64 = 0.15;
/// Eigenes Radar an: ein kleiner Verlust wie der offene Schacht
/// (Spielerentscheidung v0.2.8).
pub const SIG_RADAR: f64 = 0.15;

/// Was das Radar über das Flugzeug wissen muss.
#[derive(Debug, Clone, Copy)]
pub struct Exposure {
    pub position: DVec3,
    pub velocity: DVec3,
    /// Querlage (rad).
    pub roll: f64,
    pub height_agl_m: f64,
    /// Schachtklappen 0..1.
    pub door: f64,
    /// Eigenes Radar eingeschaltet.
    pub radar_on: bool,
    /// Am Boden: in den Bodenechos verborgen, für jedes Radar unsichtbar.
    pub on_ground: bool,
}

impl Exposure {
    /// Eigene Sichtbarkeit 0..1 – ein Wert für alle Radare (wie im
    /// Original): Grundwert, Höhe über Grund, Querlage, offener Schacht,
    /// eigenes Radar. Am Boden 0.
    pub fn signature(&self) -> f64 {
        if self.on_ground {
            return 0.0;
        }
        let above = (self.height_agl_m - LOW_LEVEL_M).max(0.0);
        let altitude = SIG_ALTITUDE * detmath::atan(above / ALTITUDE_SCALE_M) / std::f64::consts::FRAC_PI_2;
        let bank = detmath::wrap_pi(self.roll).abs();
        let bank = if bank > std::f64::consts::FRAC_PI_2 { std::f64::consts::PI - bank } else { bank };
        let bank = SIG_BANK * bank / std::f64::consts::FRAC_PI_2;
        let extra = SIG_BAY * self.door.clamp(0.0, 1.0) + if self.radar_on { SIG_RADAR } else { 0.0 };
        (SIG_BASE + altitude + bank + extra).clamp(0.0, 1.0)
    }

    /// Stealth-Level: 1 − Sichtbarkeit.
    pub fn stealth(&self) -> f64 {
        1.0 - self.signature()
    }

    /// Stärke des Impulses eines Radars `spec` bei `site` am Flugzeug, 0..1:
    /// linear mit der Entfernung fallend; das Dopplerradar sieht nur die
    /// Bewegung auf sich zu oder von sich weg.
    pub fn signal(&self, spec: &RadarSpec, site: DVec3) -> f64 {
        let to = self.position - site;
        let r = to.length() / spec.range_m;
        if r >= 1.0 || self.on_ground {
            return 0.0;
        }
        let base = 1.0 - r;
        let factor = match spec.kind {
            RadarKind::Pulse => 1.0,
            RadarKind::Doppler => {
                let v = self.velocity;
                let radial = if v.length() > 1.0 {
                    (v.dot(to) / (v.length() * to.length().max(1.0))).abs()
                } else {
                    0.0
                };
                BEAM_FLOOR + (1.0 - BEAM_FLOOR) * radial
            }
        };
        base * factor
    }
}

/// Eine Radarstellung mit Alarmstufe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadarSite {
    /// Index in [`crate::Sim::targets`].
    pub target: usize,
    pub spec: RadarSpec,
    /// Versatz des Antennenumlaufs (Ticks), damit nicht alle zugleich pingen.
    pub phase_ticks: u64,
    /// Alarmstufe 0..=3.
    pub alert: u8,
    /// Tick des letzten Treffers bzw. der letzten Herabstufung.
    pub last_change_tick: u64,
    /// Ort des Pings ab Stufe 2 (NED) – Ziel späterer Patrouillen.
    pub ping_point: Option<DVec3>,
}

impl RadarSite {
    pub fn sweep_ticks(&self) -> u64 {
        ((self.spec.sweep_s * f64::from(TICK_HZ)) as u64).max(1)
    }
}

/// Ein Ping, der das Flugzeug erreicht hat (für Anzeige und Klang).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ping {
    pub tick: u64,
    /// Index in [`RadarNet::sites`].
    pub site: usize,
    pub signal: f64,
    pub stealth: f64,
    /// Stärker als das Stealth-Level: Treffer.
    pub hit: bool,
}

/// Was eine Stellung meldet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadarEvent {
    /// Alarmstufe erreicht (1 entdeckt, 2 verfolgt, 3 Radar ID).
    Alert { site: usize, level: u8 },
    /// Alle Stufen abgeklungen: Kontakt verloren.
    Lost { site: usize },
}

/// Alle Radarstellungen.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RadarNet {
    pub sites: Vec<RadarSite>,
    pub last_ping: Option<Ping>,
    pub events: Vec<(u64, RadarEvent)>,
}

impl RadarNet {
    /// Stellungen aus den Zielen (Radarklassen), Umlauf je Stellung versetzt.
    pub fn new(targets: &[Target]) -> Self {
        let sites = targets
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                let spec = spec_for(t.class)?;
                let mut s = RadarSite {
                    target: i,
                    spec,
                    phase_ticks: 0,
                    alert: 0,
                    last_change_tick: 0,
                    ping_point: None,
                };
                s.phase_ticks = (i as u64).wrapping_mul(977) % s.sweep_ticks();
                Some(s)
            })
            .collect();
        Self { sites, last_ping: None, events: Vec::new() }
    }

    /// Ein Tick: Antennenumläufe, Pings, Alarmstufen. `exposure` ist `None`,
    /// wenn das Flugzeug nicht mehr fliegt.
    pub fn step(&mut self, tick: u64, targets: &[Target], exposure: Option<&Exposure>) {
        let decay_ticks = (DECAY_S * f64::from(TICK_HZ)) as u64;
        for k in 0..self.sites.len() {
            let site = self.sites[k];
            let t = &targets[site.target];
            // Zerstört: strahlt nicht mehr, vergisst alles.
            if !t.is_intact() {
                if site.alert > 0 {
                    self.sites[k].alert = 0;
                    self.sites[k].ping_point = None;
                }
                continue;
            }
            // Abklingen ohne Treffer
            if site.alert > 0 && tick.saturating_sub(site.last_change_tick) >= decay_ticks {
                let s = &mut self.sites[k];
                s.alert -= 1;
                s.last_change_tick = tick;
                if s.alert < 2 {
                    s.ping_point = None;
                }
                if s.alert == 0 {
                    self.log(tick, RadarEvent::Lost { site: k });
                }
            }
            let Some(x) = exposure else {
                continue;
            };
            if !(tick + site.phase_ticks).is_multiple_of(site.sweep_ticks()) {
                continue;
            }
            let c = t.center_ned();
            let at = DVec3::new(c[0], c[1], c[2]);
            let signal = x.signal(&site.spec, at);
            if signal <= 0.0 {
                continue;
            }
            let stealth = x.stealth();
            let hit = signal > stealth;
            self.last_ping = Some(Ping { tick, site: k, signal, stealth, hit });
            if hit {
                let s = &mut self.sites[k];
                s.last_change_tick = tick;
                if s.alert < MAX_ALERT {
                    s.alert += 1;
                    let level = s.alert;
                    if level >= 2 {
                        s.ping_point = Some(x.position);
                    }
                    self.log(tick, RadarEvent::Alert { site: k, level });
                } else {
                    s.ping_point = Some(x.position);
                }
            }
        }
    }

    /// Höchste Alarmstufe aller Stellungen.
    pub fn max_alert(&self) -> u8 {
        self.sites.iter().map(|s| s.alert).max().unwrap_or(0)
    }

    /// Jüngster gemerkter Ping-Ort (Ziel späterer Patrouillen).
    pub fn latest_ping_point(&self) -> Option<DVec3> {
        self.sites
            .iter()
            .filter(|s| s.ping_point.is_some())
            .max_by_key(|s| s.last_change_tick)
            .and_then(|s| s.ping_point)
    }

    fn log(&mut self, tick: u64, e: RadarEvent) {
        if self.events.len() >= EVENT_LOG {
            self.events.remove(0);
        }
        self.events.push((tick, e));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::DQuat;

    fn level(north: f64, heading: f64) -> Exposure {
        let attitude = DQuat::from_euler(heading, 0.0, 0.0);
        Exposure {
            position: DVec3::new(north, 0.0, -1_000.0),
            velocity: attitude.rotate(DVec3::new(200.0, 0.0, 0.0)),
            roll: 0.0,
            height_agl_m: 1_000.0,
            door: 0.0,
            radar_on: false,
            on_ground: false,
        }
    }

    const SITE: DVec3 = DVec3 { x: 0.0, y: 0.0, z: 0.0 };

    #[test]
    fn signal_falls_with_range_and_ends_at_the_limit() {
        let x = level(-10_000.0, 0.0);
        let near = x.signal(&EARLY_WARNING, SITE);
        let far = level(-40_000.0, 0.0).signal(&EARLY_WARNING, SITE);
        assert!(near > far && far > 0.0);
        assert_eq!(level(-61_000.0, 0.0).signal(&EARLY_WARNING, SITE), 0.0);
        // Am Boden in den Bodenechos verborgen
        assert_eq!(Exposure { on_ground: true, ..x }.signal(&SAM, SITE), 0.0);
    }

    #[test]
    fn doppler_is_weak_across_and_pulse_does_not_care() {
        // 10 km südlich, Kurs Nord (auf das Radar zu) bzw. Ost (quer)
        let toward = level(-10_000.0, 0.0).signal(&SAM, SITE);
        let across = level(-10_000.0, std::f64::consts::FRAC_PI_2).signal(&SAM, SITE);
        assert!(across < toward * 0.4, "{across} vs {toward}");
        let p_toward = level(-10_000.0, 0.0).signal(&EARLY_WARNING, SITE);
        let p_across = level(-10_000.0, std::f64::consts::FRAC_PI_2).signal(&EARLY_WARNING, SITE);
        assert!((p_toward - p_across).abs() < 1e-9);
    }

    /// Tief und waagerecht ist am unauffälligsten; Höhe, Querlage, Schacht und
    /// Radar kosten; am Boden 100 %.
    #[test]
    fn stealth_is_low_and_level() {
        let base = level(-10_000.0, 0.0);
        let low = Exposure { height_agl_m: 50.0, ..base };
        assert!((low.stealth() - (1.0 - SIG_BASE)).abs() < 1e-9, "tief: {}", low.stealth());
        let mid = base.stealth();
        let high = Exposure { height_agl_m: 3_000.0, ..base }.stealth();
        assert!(low.stealth() > mid && mid > high, "{} {mid} {high}", low.stealth());
        // Querlage: 90° kostet die volle Zulage, Rückenflug wie waagerecht.
        let knife = Exposure { roll: std::f64::consts::FRAC_PI_2, ..low };
        let inverted = Exposure { roll: std::f64::consts::PI, ..low };
        assert!((low.stealth() - knife.stealth() - SIG_BANK).abs() < 1e-9);
        assert!((inverted.stealth() - low.stealth()).abs() < 1e-9);
        let bay = Exposure { door: 1.0, ..low };
        let radar = Exposure { radar_on: true, ..low };
        assert!((low.stealth() - bay.stealth() - SIG_BAY).abs() < 1e-9);
        assert!((low.stealth() - radar.stealth() - SIG_RADAR).abs() < 1e-9);
        assert_eq!(Exposure { on_ground: true, door: 1.0, ..base }.stealth(), 1.0);
    }
}
