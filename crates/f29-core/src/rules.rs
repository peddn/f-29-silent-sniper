//! Regelsets (Einsatzregeln, ROE): Wie offen der Konflikt ist, bestimmt, ob
//! und wo der Gegner feuert, wie viele Bedrohungen gleichzeitig laufen und
//! was Entdeckung kostet (Spielerentscheidung v0.3; im Original die
//! Konfliktstufe, M-030). „Offener Krieg“ ist das bisherige Verhalten.

/// Konfliktlage einer Mission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Conflict {
    /// Kalter Krieg: Der Gegner fängt ab und verfolgt, feuert aber nur, wenn
    /// das Flugzeug über feindlichem Gebiet ist; Entdeckung kostet viel.
    ColdWar,
    /// Begrenzter Konflikt: Gegner feuern, aber weniger gleichzeitig.
    Limited,
    /// Offener Krieg.
    #[default]
    War,
}

/// Werte eines Regelsets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rules {
    pub label: &'static str,
    /// SAMs und Jäger feuern nur über feindlichem Gebiet.
    pub fire_only_over_hostile: bool,
    /// Feindliche Raketen höchstens so viele gleichzeitig in der Luft.
    pub max_hostile_missiles: usize,
    /// Jäger starten ihre Rakete nicht näher als so (m).
    pub fighter_min_range_m: f64,
    /// Feindliche Jäger höchstens so viele gleichzeitig in der Luft.
    pub max_airborne: usize,
    /// Punkteabzug je Entdeckung (Wertung, M-032).
    pub detection_penalty: u32,
    /// Angreifen nur, was uns entdeckt hat (Radarstellung mit Alarm, Jäger
    /// mit Kontakt); sonst ROE-Verletzung.
    pub attack_needs_detection: bool,
    /// Verschiebt das gewürfelte Pilotenkönnen der Jäger (−1 = schwächer).
    pub pilot_skill: i8,
    /// Faktor auf die Wertung (strengere Regeln sind schwerer zu fliegen).
    pub score_factor: f64,
}

const RULES: [Rules; 3] = [
    Rules {
        label: "COLD WAR",
        fire_only_over_hostile: true,
        max_hostile_missiles: 2,
        fighter_min_range_m: 1_500.0,
        max_airborne: 4,
        detection_penalty: 30,
        attack_needs_detection: true,
        pilot_skill: -1,
        score_factor: 1.5,
    },
    Rules {
        label: "LIMITED",
        fire_only_over_hostile: false,
        max_hostile_missiles: 2,
        fighter_min_range_m: 1_500.0,
        max_airborne: 4,
        detection_penalty: 10,
        attack_needs_detection: false,
        pilot_skill: 0,
        score_factor: 1.0,
    },
    Rules {
        label: "OPEN WAR",
        fire_only_over_hostile: false,
        max_hostile_missiles: 4,
        fighter_min_range_m: 1_000.0,
        max_airborne: 6,
        detection_penalty: 0,
        attack_needs_detection: false,
        pilot_skill: 1,
        score_factor: 0.75,
    },
];

impl Conflict {
    pub const ALL: [Conflict; 3] = [Conflict::ColdWar, Conflict::Limited, Conflict::War];

    pub fn rules(self) -> &'static Rules {
        &RULES[self as usize]
    }

    /// Kennwort für `--rules` und Replays.
    pub fn code(self) -> &'static str {
        match self {
            Conflict::ColdWar => "cold",
            Conflict::Limited => "limited",
            Conflict::War => "war",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.code() == code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Je offener der Konflikt, desto mehr Bedrohung und desto weniger
    /// kostet Entdeckung.
    #[test]
    fn rules_escalate_with_the_conflict() {
        let [cold, limited, war] = Conflict::ALL.map(Conflict::rules);
        assert!(
            cold.fire_only_over_hostile && !limited.fire_only_over_hostile && !war.fire_only_over_hostile
        );
        assert!(cold.detection_penalty > limited.detection_penalty);
        assert!(limited.detection_penalty > war.detection_penalty);
        assert_eq!(war.detection_penalty, 0);
        assert!(war.max_hostile_missiles > limited.max_hostile_missiles);
        assert!(war.max_airborne > limited.max_airborne);
        assert!(cold.pilot_skill < limited.pilot_skill && limited.pilot_skill < war.pilot_skill);
        assert!(cold.score_factor > limited.score_factor && limited.score_factor > war.score_factor);
        for c in Conflict::ALL {
            assert_eq!(Conflict::from_code(c.code()), Some(c));
        }
    }
}
