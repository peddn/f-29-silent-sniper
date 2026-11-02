//! Wertung (M-032): Punkte je Eintrag des Flugprotokolls, dazu Faktoren für
//! Ausgang, Regelset und Flugzeit. Aufbau wie im Original, eigene Werte.

use super::{DT, Sim, Status};
use crate::flightlog::{Detector, LogEntry};
use crate::mission::Task;
use f29_world::{Side, TargetClass};

/// Primärziel erledigt.
pub const PRIMARY: i32 = 500;
/// Sekundärziel erledigt; mit erledigtem Primärziel zählt es [`SECONDARY_BOTH`].
pub const SECONDARY: i32 = 200;
pub const SECONDARY_BOTH: i32 = 400;
/// Andere zerstörte Ziele: Radar- und Flugabwehrstellungen, sonst militärisch,
/// zivil.
pub const RADAR_SITE: i32 = 50;
pub const MILITARY: i32 = 25;
pub const CIVILIAN: i32 = -200;
/// Jägerabschüsse (MiG-21, MiG-29).
pub const FIGHTERS: [i32; 2] = [100, 150];
/// Wegpunkt außerhalb des eigenen Gebiets.
pub const WAYPOINT: i32 = 5;
/// Verletzung der Einsatzregeln.
pub const ROE: i32 = -100;
/// Flugzeit ab so vielen Minuten: Faktor (der Treibstoff reicht nur für
/// weite Wege, nicht fürs Trödeln).
pub const TIME_BONUS: [(u64, f64); 3] = [(20, 1.1), (30, 1.2), (40, 1.3)];

/// Eine Zeile der Wertung.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreLine {
    /// Zeit seit Missionsstart (s).
    pub time_s: u64,
    pub text: String,
    pub points: i32,
}

/// Die Wertung einer Mission.
#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    pub lines: Vec<ScoreLine>,
    /// Summe der Zeilen, mindestens 0.
    pub subtotal: i32,
    /// Faktoren (Bezeichnung, Wert), der Reihe nach angewandt.
    pub factors: Vec<(String, f64)>,
    pub total: i32,
}

impl Sim {
    /// Wertung nach dem Flugprotokoll bis jetzt.
    pub fn score(&self) -> Score {
        let rules = self.rules();
        let objective = |i: usize, task: Task| self.mission.objectives().position(|o| o == (i, task));
        let mut done = [false; 2];
        let mut lines = Vec::new();
        for (k, &(tick, e)) in self.log.iter().enumerate() {
            // Zerstört man, was einen entdeckt hat, gibt es den Abzug zurück.
            let seen_by = |d: Detector| self.log[..k].iter().any(|x| x.1 == LogEntry::Detected(d));
            let refund = |d: Detector| if seen_by(d) { rules.detection_penalty as i32 } else { 0 };
            let mut goal = |slot: usize| {
                done[slot] = true;
                match slot {
                    0 if done[1] => PRIMARY + SECONDARY_BOTH - SECONDARY,
                    0 => PRIMARY,
                    _ if done[0] => SECONDARY_BOTH,
                    _ => SECONDARY,
                }
            };
            let points = match e {
                LogEntry::Photographed(slot) => goal(slot),
                LogEntry::Destroyed(i) => {
                    let t = &self.targets[i];
                    let base = match objective(i, Task::Attack) {
                        Some(slot) => goal(slot),
                        None if t.civilian => CIVILIAN,
                        None if matches!(
                            t.class,
                            TargetClass::AirDefense | TargetClass::EarlyWarning | TargetClass::ShortRangeSam
                        ) =>
                        {
                            RADAR_SITE
                        }
                        None => MILITARY,
                    };
                    base + refund(Detector::Radar(i))
                }
                LogEntry::FighterKill(id, kind) => {
                    FIGHTERS[kind as usize] + refund(Detector::Fighter(id, kind))
                }
                LogEntry::Waypoint(w) => {
                    let p = &self.route[w];
                    if self.config.theater.side_at(p.north, p.east) == Side::Friendly { 0 } else { WAYPOINT }
                }
                LogEntry::Detected(_) => -(rules.detection_penalty as i32),
                LogEntry::RoeViolation => ROE,
                _ => 0,
            };
            lines.push(ScoreLine { time_s: (tick as f64 * DT) as u64, text: self.log_text(e), points });
        }
        let subtotal = lines.iter().map(|l| l.points).sum::<i32>().max(0);
        let mut factors = vec![(self.status.outcome(), outcome_factor(self.status))];
        factors.push((format!("RULES {}", rules.label), rules.score_factor));
        let sp = &self.config.spawn;
        factors.push((format!("OPPONENTS {}", sp.opponents.label()), sp.opponents.score_factor()));
        factors.push((sp.landings.label().to_string(), sp.landings.score_factor()));
        let start = self.logged(LogEntry::Takeoff).unwrap_or(0);
        let minutes =
            (self.status.tick().unwrap_or(self.tick).saturating_sub(start) as f64 * DT / 60.0) as u64;
        if let Some(&(m, f)) = TIME_BONUS.iter().rev().find(|(m, _)| minutes >= *m) {
            factors.push((format!("FLIGHT TIME {m}+ MIN"), f));
        }
        let total = (factors.iter().fold(f64::from(subtotal), |s, (_, f)| s * f)).round() as i32;
        Score { lines, subtotal, factors, total }
    }
}

/// Faktor je Ausgang: heil zurück voll, ausgestiegen über eigenem Gebiet
/// halb, neutrales Gebiet ein Viertel, gefangen oder tot nichts.
pub fn outcome_factor(status: Status) -> f64 {
    let side = |s: Side| match s {
        Side::Friendly => 0.5,
        Side::Neutral => 0.25,
        Side::Hostile => 0.0,
    };
    match status {
        Status::Landed { side: Side::Friendly, .. } => 1.0,
        Status::Landed { side: Side::Neutral, .. } => 0.25,
        Status::Ejected { side: s, survived: true, .. } => side(s),
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SimConfig;

    /// Eine Mission mit Angriff auf Primär- und Sekundärziel, Protokoll von Hand.
    fn sim_with(log: &[LogEntry], status: Status) -> Sim {
        let mut sim = Sim::new(SimConfig::new(3));
        sim.mission.tasks = [Task::Attack, Task::Attack];
        sim.log = log.iter().enumerate().map(|(k, &e)| (k as u64 * 120, e)).collect();
        sim.status = status;
        sim
    }

    fn index(sim: &Sim, f: impl Fn(&crate::targets::Target) -> bool) -> usize {
        let goal = [sim.mission.primary, sim.mission.secondary];
        (0..sim.targets.len()).find(|&i| f(&sim.targets[i]) && !goal.contains(&Some(i))).expect("Ziel")
    }

    const HOME: Status = Status::Landed { tick: 1, side: Side::Friendly };

    /// Das Sekundärziel zählt mehr, sobald auch das Primärziel fällt – auch
    /// wenn es erst danach fällt.
    #[test]
    fn secondary_counts_more_with_the_primary() {
        let probe = sim_with(&[], HOME);
        let (p, s) = (probe.mission.primary.unwrap(), probe.mission.secondary.unwrap());
        let sim = sim_with(&[LogEntry::Destroyed(s), LogEntry::Destroyed(p)], HOME);
        let score = sim.score();
        let points: Vec<i32> = score.lines.iter().map(|l| l.points).collect();
        assert_eq!(points, [SECONDARY, PRIMARY + SECONDARY_BOTH - SECONDARY]);
        assert_eq!(score.subtotal, PRIMARY + SECONDARY_BOTH);
    }

    /// Entdeckung kostet; wer den Entdecker zerstört, bekommt den Abzug
    /// zurück. Zivile Ziele kosten, die Summe bleibt mindestens 0.
    #[test]
    fn detection_refund_and_civilians() {
        let probe = sim_with(&[], HOME);
        let site = probe.radar.sites[0].target;
        let civ = index(&probe, |t| t.civilian);
        let pen = probe.rules().detection_penalty as i32;
        let d = LogEntry::Detected(Detector::Radar(site));
        let sim = sim_with(&[d, LogEntry::Destroyed(site)], HOME);
        let points: Vec<i32> = sim.score().lines.iter().map(|l| l.points).collect();
        assert_eq!(points, [-pen, RADAR_SITE + pen]);
        let sim = sim_with(&[LogEntry::Destroyed(civ)], HOME);
        assert_eq!(sim.score().lines[0].points, CIVILIAN);
        assert_eq!(sim.score().subtotal, 0);
    }

    /// Faktoren: Ausgang, Regelset, Flugzeit.
    #[test]
    fn factors_follow_outcome_rules_and_flight_time() {
        let probe = sim_with(&[], HOME);
        let p = probe.mission.primary.unwrap();
        let rules = probe.rules().score_factor;
        let landed = Status::Landed { tick: (25.0 * 60.0 / DT) as u64, side: Side::Friendly };
        let score = sim_with(&[LogEntry::Takeoff, LogEntry::Destroyed(p)], landed).score();
        assert_eq!(score.factors.len(), 5, "{:?}", score.factors);
        assert_eq!(score.total, (f64::from(PRIMARY) * rules * 1.1).round() as i32);
        // Gegnerqualität und Flugverhalten zählen mit.
        let mut sim = sim_with(&[LogEntry::Destroyed(p)], HOME);
        sim.config.spawn.opponents = crate::rules::Opponents::Elite;
        sim.config.spawn.landings = crate::rules::Landings::Easy;
        assert_eq!(sim.score().total, (f64::from(PRIMARY) * rules * 1.5 * 0.9).round() as i32);
        let captured = Status::Ejected { tick: 1, side: Side::Hostile, survived: true };
        assert_eq!(sim_with(&[LogEntry::Destroyed(p)], captured).score().total, 0);
        let rescued = Status::Ejected { tick: 1, side: Side::Friendly, survived: true };
        assert_eq!(outcome_factor(rescued), 0.5);
    }
}
