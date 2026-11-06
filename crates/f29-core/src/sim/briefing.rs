//! Briefingtext vor dem Flug (M-036): Aufträge, Bedrohungen, Flugplan –
//! alles, um eine Mission vor dem Start einzuschätzen (und sie gegen eine
//! neue zu tauschen). Die Lagebesprechung zeigt die Teile einzeln neben der
//! Karte, die Schlussauswahl die Übersicht.

use super::Sim;
use crate::detmath;
use crate::targets::{TargetClass, air_defense};
use crate::terrain::AirfieldKind;
use crate::weapons::{TANK_FUEL_KG, WeaponKind};

/// Kilometer, gerundet.
fn km(m: f64) -> u32 {
    (m / 1_000.0).round() as u32
}

impl Sim {
    /// Treibstoffbedarf der Route samt Reserve (kg).
    pub fn fuel_required_kg(&self) -> f64 {
        super::fuel_required_kg(&self.route)
    }

    /// Mitgeführter Treibstoff: Haupttanks plus volle Zusatztanks (kg).
    pub fn fuel_carried_kg(&self) -> f64 {
        let tanks = self.stores.bays.iter().filter(|b| b.kind == Some(WeaponKind::DropTank) && b.count > 0);
        self.aircraft.fuel_kg + tanks.count() as f64 * TANK_FUEL_KG
    }

    /// Entfernung (km) und rechtweisende Richtung (Grad) von der Basis.
    fn home_vector(&self, north: f64, east: f64) -> (u32, u32) {
        let home = &self.route[0];
        let (dn, de) = (north - home.north, east - home.east);
        let deg = detmath::atan2(de, dn).to_degrees().rem_euclid(360.0).round() as u32 % 360;
        (km(dn.hypot(de)), deg)
    }

    /// Aufträge: je Ziel Art, Name, Entfernung und Richtung; Kamera.
    pub fn briefing_targets(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (k, (i, task)) in self.mission.objectives().enumerate() {
            let t = &self.targets[i];
            let (dist, deg) = self.home_vector(t.north, t.east);
            lines.push(format!("{}: {} {}", ["PRIMARY", "SECONDARY"][k], task.label(), t.name));
            let grid = self.grid_square(t.north, t.east);
            lines.push(format!("    GRID {grid}, {dist} KM FROM {}, BEARING {deg:03}", self.route[0].name));
        }
        if self.mission.has_recon() {
            lines.push(String::new());
            lines.push("CAMERA REQUIRED IN ONE BAY".into());
        }
        lines
    }

    /// Bedrohungen: Einsatzregeln, Stellungen je Typ mit Radar- und
    /// Raketenreichweite, feindliche Flugplätze.
    pub fn briefing_threats(&self) -> Vec<String> {
        let mut lines = self.rules_lines();
        lines.push(String::new());
        lines.push("AIR DEFENSE ALONG THE ROUTE".into());
        for class in [TargetClass::EarlyWarning, TargetClass::AirDefense, TargetClass::ShortRangeSam] {
            let Some((radar, missile)) = air_defense(class) else { continue };
            let near = |n: f64, e: f64| self.route_distance_m(n, e) <= radar.range_m;
            let n = self
                .targets
                .iter()
                .filter(|t| t.class == class && t.is_intact() && near(t.north, t.east))
                .count();
            let (r, m) = (km(radar.range_m), km(missile.spec().max_range_m));
            lines.push(format!(
                "{n:>2} {:<12}RADAR {r:>2} KM  {:<6}{m:>3} KM",
                class.label(),
                missile.label()
            ));
        }
        lines.push(String::new());
        lines.push("FIGHTER BASES".into());
        for rw in self.terrain.runways().iter().filter(|r| r.kind == AirfieldKind::Enemy) {
            let (dist, deg) = self.home_vector(rw.center[0], rw.center[1]);
            lines.push(format!("    {:<8}{dist:>4} KM, BEARING {deg:03}", rw.name));
        }
        lines
    }

    /// Flugplan: Wegpunkte mit Teilstrecken, Gesamtstrecke, Treibstoff,
    /// Tageszeit.
    /// Planquadrat eines Punkts im Gebiet (M-031).
    pub fn grid_square(&self, north: f64, east: f64) -> String {
        crate::nav::grid_square(north, east, crate::nav::HALF_EXTENT_M)
    }

    /// Je Wegpunkt Planquadrat, Strecke und Verbrauch des Abschnitts nach der
    /// Planung (Reiseverbrauch, M-009), darunter der Rest nach der Route.
    pub fn briefing_route(&self) -> Vec<String> {
        let kg = |x: f64| (x / 10.0).round() as i64 * 10;
        let mut lines = Vec::new();
        let mut left = self.fuel_carried_kg();
        for (k, sp) in self.route.iter().enumerate() {
            let leg = k.checked_sub(1).map(|j| {
                let p = &self.route[j];
                let m = (sp.north - p.north).hypot(sp.east - p.east);
                let burn = m / 1_000.0 * super::CRUISE_FUEL_KG_PER_KM;
                left -= burn;
                format!("{:>4} KM{:>8} KG", km(m), kg(burn))
            });
            let name: String = sp.name.chars().take(14).collect();
            let grid = self.grid_square(sp.north, sp.east);
            lines.push(format!("{} {name:<15}{grid:<5}{}", k + 1, leg.unwrap_or_default()).trim_end().into());
        }
        lines.push(format!("  {:<20}{:>15} KG", "LEFT", kg(left)));
        lines.push(String::new());
        lines.extend(self.route_lines());
        lines
    }

    /// Übersicht: Aufträge, Strecke und Treibstoff, Regeln, Tageszeit.
    pub fn briefing(&self) -> Vec<String> {
        let mut lines = self.briefing_targets();
        lines.push(String::new());
        let [route, fuel, time] = self.route_lines();
        lines.extend([route, fuel]);
        lines.extend(self.rules_lines());
        lines.push(time);
        lines
    }

    /// Kleinster Abstand eines Punkts von der Route (m).
    fn route_distance_m(&self, north: f64, east: f64) -> f64 {
        let legs = self.route.windows(2).map(|w| {
            let (a, b) = ([w[0].north, w[0].east], [w[1].north, w[1].east]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let t =
                (((north - a[0]) * dx + (east - a[1]) * dy) / (dx * dx + dy * dy).max(1.0)).clamp(0.0, 1.0);
            (north - a[0] - t * dx).hypot(east - a[1] - t * dy)
        });
        legs.fold(f64::MAX, f64::min)
    }

    /// Gesamtstrecke, Treibstoff, Tageszeit.
    fn route_lines(&self) -> [String; 3] {
        let (need, have) = (self.fuel_required_kg(), self.fuel_carried_kg());
        [
            format!("ROUTE {} KM", km(crate::nav::route_length_m(&self.route))),
            format!("FUEL ESTIMATE {need:.0} KG, CARRIED {have:.0} KG"),
            format!("TIME: {}", if self.night() { "NIGHT" } else { "DAY" }),
        ]
    }

    /// Einsatzregeln des Regelsets.
    fn rules_lines(&self) -> Vec<String> {
        let rules = self.rules();
        let mut lines = vec![format!("RULES: {}", rules.label)];
        if rules.attack_needs_detection {
            lines.push("    ATTACK ONLY WHAT HAS DETECTED YOU".into());
        }
        if rules.fire_only_over_hostile {
            lines.push("    ENEMY FIRES ONLY OVER HOSTILE TERRITORY".into());
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::Conflict;
    use crate::{Sim, SimConfig};

    /// Das Briefing nennt beide Aufträge, die Strecke und im Kalten Krieg
    /// Kamera und Einsatzregeln.
    #[test]
    fn briefing_names_tasks_route_and_rules() {
        let mut c = SimConfig::new(1);
        c.spawn.conflict = Conflict::ColdWar;
        let sim = Sim::new(c);
        let text = sim.briefing().join("\n");
        for part in [
            "PRIMARY: RECON",
            "SECONDARY: RECON",
            "ROUTE",
            "FUEL ESTIMATE",
            "CAMERA REQUIRED",
            "DETECTED YOU",
            "TIME: DAY",
        ] {
            assert!(text.contains(part), "{part} fehlt:\n{text}");
        }
        let threats = sim.briefing_threats().join("\n");
        for part in ["EW RADAR", "SA-6", "FIGHTER BASES", "RED 1"] {
            assert!(threats.contains(part), "{part} fehlt:\n{threats}");
        }
        let route = sim.briefing_route();
        assert!(route[0].starts_with("1 HOME") && route[1].ends_with(" KG"), "{route:?}");
        let home = &sim.route[0];
        assert!(route[0].ends_with(&sim.grid_square(home.north, home.east)), "{route:?}");
        assert!(text.contains("GRID "), "Planquadrat fehlt:\n{text}");
        // Verbrauch je Abschnitt, zusammen der Reiseverbrauch; darunter der Rest.
        let kg = |line: &String| line.split_whitespace().rev().nth(1).unwrap().parse::<f64>().unwrap();
        let n = sim.route.len();
        let burn: f64 = route[1..n].iter().map(kg).sum();
        let planned = sim.fuel_required_kg() - crate::sim::FUEL_RESERVE_KG;
        assert!((burn - planned).abs() < 30.0, "Verbrauch {burn} statt {planned}");
        let left = sim.fuel_carried_kg() - planned;
        assert!(route[n].starts_with("  LEFT") && (kg(&route[n]) - left).abs() < 10.0, "{route:?}");
    }
}

#[cfg(test)]
mod tank_tests {
    use crate::controls::{ControlFrame, weapon_flags};
    use crate::sim::Spawn;
    use crate::weapons::{Loadout, TANK_FUEL_KG, WeaponKind};
    use crate::{Sim, SimConfig};

    /// Der Zusatztank pumpt ohne Klappen um, der Schacht ist danach leer;
    /// was nicht in die Haupttanks passt, geht verloren.
    #[test]
    fn drop_tank_transfers_fuel_without_doors() {
        let tank = Some(Loadout([Some(WeaponKind::DropTank), None, None, None]));
        let fly = |fuel: f64| {
            let mut sim = Sim::new(SimConfig::with(1, Spawn { fuel, loadout: tank, ..Spawn::default() }));
            let (f0, m0) = (sim.aircraft.fuel_kg, sim.aircraft.stores_kg);
            let c = ControlFrame::from_analog(0.0, 0.0, 0.0, 0.5, 0);
            sim.step(c.with_weapons(weapon_flags::NEXT_BAY));
            sim.step(c.with_weapons(weapon_flags::RELEASE));
            assert_eq!(sim.stores.bays[0].kind, None, "Schacht leer");
            assert!(sim.stores.door < 0.01, "Klappen bleiben zu");
            assert!(sim.aircraft.stores_kg < m0 - TANK_FUEL_KG);
            sim.aircraft.fuel_kg - f0
        };
        assert!((fly(0.5) - TANK_FUEL_KG).abs() < 5.0, "voller Inhalt umgepumpt");
        assert!(fly(0.95) < TANK_FUEL_KG * 0.5, "Überschuss verloren");
    }

    /// Reicht der Haupttank für die Route nicht, schlägt die Beladung einen
    /// Zusatztank vor (wie im Original); sonst nicht.
    #[test]
    fn long_routes_suggest_a_drop_tank() {
        let mut seen = [false; 2];
        for seed in 1..40 {
            let sim =
                Sim::new(SimConfig::with(seed, Spawn { loadout: None, ..SimConfig::runway(seed).spawn }));
            let tank = sim.stores.bays.iter().any(|b| b.kind == Some(WeaponKind::DropTank));
            assert_eq!(tank, sim.fuel_required_kg() > sim.aircraft.fuel_kg, "Seed {seed}");
            seen[usize::from(tank)] = true;
        }
        assert_eq!(seen, [true, true], "beide Fälle kommen vor");
    }
}
