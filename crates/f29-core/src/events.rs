//! Ereignisprotokoll: die letzten `N` Ereignisse mit Tick. Die Teilsysteme
//! (Waffen, Radar, Kanone, Jäger) sammeln ihre Ereignisse eines Ticks darin;
//! die Simulation übernimmt sie in ihren einen Ereignisstrom
//! ([`crate::sim::Event`]), aus dem Meldungen, Klang, Effekte und das
//! Flugprotokoll entstehen.

use std::ops::Deref;

/// Die letzten `N` Ereignisse, älteste zuerst.
#[derive(Debug, Clone, PartialEq)]
pub struct EventLog<T, const N: usize = 16>(Vec<(u64, T)>);

impl<T, const N: usize> Default for EventLog<T, N> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T, const N: usize> EventLog<T, N> {
    pub fn push(&mut self, tick: u64, e: T) {
        if self.0.len() >= N {
            self.0.remove(0);
        }
        self.0.push((tick, e));
    }

    /// Alle Ereignisse herausnehmen.
    pub fn take(&mut self) -> Vec<(u64, T)> {
        std::mem::take(&mut self.0)
    }

    /// Ereignisse der letzten `window` Ticks bis `now`.
    pub fn recent(&self, now: u64, window: u64) -> impl Iterator<Item = &(u64, T)> {
        self.0.iter().filter(move |(t, _)| now.saturating_sub(*t) <= window)
    }
}

impl<T, const N: usize> Deref for EventLog<T, N> {
    type Target = [(u64, T)];

    fn deref(&self) -> &[(u64, T)] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_newest_and_filters_by_age() {
        let mut log: EventLog<u64> = EventLog::default();
        for t in 0..20_u64 {
            log.push(t, t * 10);
        }
        assert_eq!(log.len(), 16);
        assert_eq!(log[0], (4, 40));
        let recent: Vec<_> = log.recent(19, 2).map(|(t, _)| *t).collect();
        assert_eq!(recent, [17, 18, 19]);
    }
}
