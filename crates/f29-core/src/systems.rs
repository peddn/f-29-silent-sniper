//! Bordsysteme und ihr Zustand – Gerüst für das Schadensmodell (Etappe 4).
//!
//! Jedes System ist intakt, eingeschränkt (mit verbleibender Leistung 0..1)
//! oder ausgefallen. Vorerst ist alles intakt; das Schadensmodell setzt die
//! Zustände später über [`crate::Sim::set_condition`] (und einen eigenen
//! Zufallsstrom). Die Wirkung steckt schon jetzt an genau einer Stelle je
//! System:
//!
//! | System | Wirkung |
//! |---|---|
//! | Triebwerk | Höchstschub × Leistung, ausgefallen = kein Schub; nach Raketentreffern 4 % je Treffer weniger |
//! | Treibstoff | Leck: bis [`FUEL_LEAK_MAX_KGPS`] zusätzlich |
//! | Hydraulik | Klappen, Fahrwerk, Schachtklappen langsamer; ausgefallen = blockiert |
//! | Flugsteuerung | Drehraten × Leistung, ausgefallen = Notsteuerung ([`CONTROL_BACKUP`]) |
//! | Radar | Reichweite × Leistung, ausgefallen = aus und nicht einschaltbar |
//! | Optik | Reichweite × Leistung, ausgefallen = keine optische Erfassung |
//! | Waffenschacht | ausgefallen = Klappen klemmen, kein Auslösen |
//! | Anzeigen | HUD und MFDs flackern, ausgefallen = dunkel |

/// Treibstoffleck eines ausgefallenen Tanksystems (kg/s).
pub const FUEL_LEAK_MAX_KGPS: f64 = 3.0;
/// Verbleibende Ruderwirkung bei ausgefallener Flugsteuerung.
pub const CONTROL_BACKUP: f64 = 0.3;

/// Restleistung eines Systems nach dem ersten Treffer.
pub const HIT_CAPABILITY: f64 = 0.5;
/// Ist das Triebwerk beschädigt, kostet jeder Raketentreffer so viel
/// Höchstschub (wie im Original: 4 % je Treffer, ausfallen kann es nicht).
pub const THRUST_LOSS_PER_HIT: f64 = 0.04;

/// Ein Bordsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum System {
    Engine,
    Fuel,
    Hydraulics,
    FlightControls,
    Radar,
    Optics,
    WeaponBay,
    Displays,
}

impl System {
    pub const ALL: [System; 8] = [
        System::Engine,
        System::Fuel,
        System::Hydraulics,
        System::FlightControls,
        System::Radar,
        System::Optics,
        System::WeaponBay,
        System::Displays,
    ];

    /// Kurzname für die SYS-Seite.
    pub fn label(self) -> &'static str {
        match self {
            System::Engine => "ENG",
            System::Fuel => "FUEL",
            System::Hydraulics => "HYD",
            System::FlightControls => "FCS",
            System::Radar => "RDR",
            System::Optics => "OPT",
            System::WeaponBay => "BAY",
            System::Displays => "DSP",
        }
    }
}

/// Zustand eines Systems.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Condition {
    #[default]
    Ok,
    /// Eingeschränkt, verbleibende Leistung 0..1.
    Degraded(f64),
    Failed,
}

impl Condition {
    /// Verbleibende Leistung 0..1.
    pub fn capability(self) -> f64 {
        match self {
            Condition::Ok => 1.0,
            Condition::Degraded(c) => c.clamp(0.0, 1.0),
            Condition::Failed => 0.0,
        }
    }

    pub fn is_ok(self) -> bool {
        self == Condition::Ok
    }

    pub fn is_failed(self) -> bool {
        self == Condition::Failed
    }
}

/// Wirkung der Systemzustände auf das Flugmodell (vom Sim vor jedem Tick
/// gesetzt, intakt = alles 1 bzw. kein Leck).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Health {
    /// Faktor auf den Höchstschub.
    pub thrust: f64,
    /// Faktor auf die Drehraten der Flugsteuerung.
    pub control: f64,
    /// Faktor auf die Stellgeschwindigkeit von Klappen und Fahrwerk (0 = blockiert).
    pub hydraulics: f64,
    /// Zusätzlicher Treibstoffverlust (kg/s).
    pub fuel_leak_kgps: f64,
}

impl Default for Health {
    fn default() -> Self {
        Self { thrust: 1.0, control: 1.0, hydraulics: 1.0, fuel_leak_kgps: 0.0 }
    }
}

/// Zustand aller Bordsysteme.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Systems {
    conditions: [Condition; 8],
    /// Raketentreffer bisher.
    pub hits: u32,
}

impl Systems {
    pub fn get(&self, s: System) -> Condition {
        self.conditions[s as usize]
    }

    pub fn set(&mut self, s: System, c: Condition) {
        self.conditions[s as usize] = c;
    }

    pub fn capability(&self, s: System) -> f64 {
        self.get(s).capability()
    }

    /// Ein Raketentreffer auf die Systeme `hit`: intakt → halbe Leistung,
    /// eingeschränkt → ausgefallen. Das Triebwerk ist wie im Original nur
    /// „beschädigt“: Dann kostet jeder Treffer – auch spätere auf andere
    /// Systeme – [`THRUST_LOSS_PER_HIT`] Höchstschub.
    pub fn missile_hit(&mut self, hit: &[System]) {
        self.hits += 1;
        for &s in hit {
            let next = match self.get(s) {
                _ if s == System::Engine => Condition::Degraded(1.0),
                Condition::Ok => Condition::Degraded(HIT_CAPABILITY),
                _ => Condition::Failed,
            };
            self.set(s, next);
        }
        if !self.get(System::Engine).is_ok() {
            let thrust = (1.0 - THRUST_LOSS_PER_HIT * f64::from(self.hits)).max(0.0);
            self.set(System::Engine, Condition::Degraded(thrust));
        }
    }

    /// Mindestens ein System ist nicht intakt.
    pub fn any_damage(&self) -> bool {
        self.conditions.iter().any(|c| !c.is_ok())
    }

    /// Wirkung auf das Flugmodell.
    pub fn health(&self) -> Health {
        let fcs = self.capability(System::FlightControls);
        Health {
            thrust: self.capability(System::Engine),
            control: CONTROL_BACKUP + (1.0 - CONTROL_BACKUP) * fcs,
            hydraulics: self.capability(System::Hydraulics),
            fuel_leak_kgps: FUEL_LEAK_MAX_KGPS * (1.0 - self.capability(System::Fuel)),
        }
    }

    /// Geschwindigkeitsfaktor der Schachtklappen (0 = klemmen).
    pub fn bay_door_speed(&self) -> f64 {
        if self.get(System::WeaponBay).is_failed() { 0.0 } else { self.capability(System::Hydraulics) }
    }

    /// Für den Zustandshash: je System (Art, Leistung), dazu die Trefferzahl.
    pub fn hash_parts(&self) -> impl Iterator<Item = (u64, f64)> + '_ {
        self.conditions
            .iter()
            .map(|c| match c {
                Condition::Ok => (0, 1.0),
                Condition::Degraded(v) => (1, *v),
                Condition::Failed => (2, 0.0),
            })
            .chain(std::iter::once((3, f64::from(self.hits))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intact_aircraft_has_full_health() {
        let s = Systems::default();
        assert!(!s.any_damage());
        assert_eq!(s.health(), Health::default());
        assert_eq!(s.bay_door_speed(), 1.0);
    }

    #[test]
    fn conditions_map_to_effects() {
        let mut s = Systems::default();
        s.set(System::Engine, Condition::Degraded(0.6));
        s.set(System::FlightControls, Condition::Failed);
        s.set(System::Fuel, Condition::Degraded(0.5));
        s.set(System::WeaponBay, Condition::Failed);
        let h = s.health();
        assert_eq!(h.thrust, 0.6);
        assert_eq!(h.control, CONTROL_BACKUP);
        assert!((h.fuel_leak_kgps - FUEL_LEAK_MAX_KGPS * 0.5).abs() < 1e-12);
        assert_eq!(s.bay_door_speed(), 0.0);
        assert!(s.any_damage());
        for sys in System::ALL {
            assert!(!sys.label().is_empty());
        }
    }

    /// Wie im Original: Erst ein Treffer am Triebwerk kostet Schub, dann
    /// jeder Treffer 4 % – auch frühere zählen mit; ausfallen kann es nicht.
    #[test]
    fn missile_hits_cost_thrust_once_the_engine_is_damaged() {
        let mut s = Systems::default();
        s.missile_hit(&[System::Radar, System::Optics]);
        assert_eq!(s.health().thrust, 1.0);
        assert_eq!(s.get(System::Radar), Condition::Degraded(HIT_CAPABILITY));
        s.missile_hit(&[System::Engine, System::Radar]);
        assert!((s.health().thrust - 0.92).abs() < 1e-12);
        assert!(s.get(System::Radar).is_failed());
        s.missile_hit(&[System::Engine, System::Fuel]);
        s.missile_hit(&[System::Hydraulics, System::Displays]);
        assert!((s.health().thrust - 0.84).abs() < 1e-12);
        assert_eq!(s.hits, 4);
    }
}
