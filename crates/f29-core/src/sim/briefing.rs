//! Briefingtext vor dem Flug (M-036): Aufträge, Entfernungen, Strecke,
//! Kamera, Einsatzregeln, Tageszeit – alles, um eine Mission vor dem Start
//! einzuschätzen (und sie gegen eine neue zu tauschen).

use super::Sim;
use crate::weapons::{TANK_FUEL_KG, WeaponKind};

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

    /// Zeilen des Briefings.
    pub fn briefing(&self) -> Vec<String> {
        let km = |m: f64| (m / 1_000.0).round() as u32;
        let home = &self.route[0];
        let mut lines = Vec::new();
        for (k, (i, task)) in self.mission.objectives().enumerate() {
            let t = &self.targets[i];
            let from = km((t.north - home.north).hypot(t.east - home.east));
            let label = ["PRIMARY", "SECONDARY"][k];
            lines.push(format!("{label}: {} {}", task.label(), t.name));
            lines.push(format!("    {from} KM FROM {}", home.name));
        }
        lines.push(String::new());
        lines.push(format!("ROUTE {} KM", km(crate::nav::route_length_m(&self.route))));
        let (need, have) = (self.fuel_required_kg(), self.fuel_carried_kg());
        lines.push(format!("FUEL ESTIMATE {need:.0} KG, CARRIED {have:.0} KG"));
        if self.mission.has_recon() {
            lines.push("CAMERA REQUIRED IN ONE BAY".into());
        }
        let rules = self.rules();
        lines.push(format!("RULES: {}", rules.label));
        if rules.attack_needs_detection {
            lines.push("    ATTACK ONLY WHAT HAS DETECTED YOU".into());
        }
        if rules.fire_only_over_hostile {
            lines.push("    ENEMY FIRES ONLY OVER HOSTILE TERRITORY".into());
        }
        lines.push(format!("TIME: {}", if self.night() { "NIGHT" } else { "DAY" }));
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
        let text = Sim::new(c).briefing().join("\n");
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
