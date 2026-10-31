//! Ereignisprotokoll eines Teilsystems: die letzten Ereignisse mit Tick, für
//! Meldungen und Klang. Gemeinsam für Waffen, Radar und Kanone.

use std::ops::Deref;

/// So viele Ereignisse bleiben im Protokoll.
const CAPACITY: usize = 16;

/// Die letzten [`CAPACITY`] Ereignisse, älteste zuerst.
#[derive(Debug, Clone, PartialEq)]
pub struct EventLog<T>(Vec<(u64, T)>);

impl<T> Default for EventLog<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T> EventLog<T> {
    pub fn push(&mut self, tick: u64, e: T) {
        if self.0.len() >= CAPACITY {
            self.0.remove(0);
        }
        self.0.push((tick, e));
    }

    /// Ereignisse der letzten `window` Ticks bis `now`.
    pub fn recent(&self, now: u64, window: u64) -> impl Iterator<Item = &(u64, T)> {
        self.0.iter().filter(move |(t, _)| now.saturating_sub(*t) <= window)
    }
}

impl<T> Deref for EventLog<T> {
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
        let mut log = EventLog::default();
        for t in 0..20_u64 {
            log.push(t, t * 10);
        }
        assert_eq!(log.len(), CAPACITY);
        assert_eq!(log[0], (4, 40));
        let recent: Vec<_> = log.recent(19, 2).map(|(t, _)| *t).collect();
        assert_eq!(recent, [17, 18, 19]);
    }
}
