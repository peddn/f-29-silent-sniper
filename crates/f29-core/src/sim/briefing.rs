//! Briefingtext vor dem Flug (M-036): Aufträge, Entfernungen, Strecke,
//! Kamera, Einsatzregeln, Tageszeit – alles, um eine Mission vor dem Start
//! einzuschätzen (und sie gegen eine neue zu tauschen).

use super::Sim;

impl Sim {
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
        let route: f64 =
            self.route.windows(2).map(|w| (w[1].north - w[0].north).hypot(w[1].east - w[0].east)).sum();
        lines.push(String::new());
        lines.push(format!("ROUTE {} KM", km(route)));
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
        for part in
            ["PRIMARY: RECON", "SECONDARY: RECON", "ROUTE", "CAMERA REQUIRED", "DETECTED YOU", "TIME: DAY"]
        {
            assert!(text.contains(part), "{part} fehlt:\n{text}");
        }
    }
}
