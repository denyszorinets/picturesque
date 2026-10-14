//! When watched sources are scanned: after a change has settled, and
//! periodically.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// How long a watched folder must be quiet after a change before it is
/// scanned, so files being copied in are complete.
pub(super) const SETTLE: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub(super) struct Schedule {
    /// Last change seen per source.
    changes: HashMap<i64, Instant>,
    last_rescan: Instant,
}

/// What is due now.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Due {
    /// Sources whose changes have settled.
    pub changed: Vec<i64>,
    /// Every watched source is due for its periodic scan.
    pub rescan: bool,
    /// How long until something else is due.
    pub wait: Option<Duration>,
}

impl Schedule {
    pub fn new(now: Instant) -> Self {
        Self {
            changes: HashMap::new(),
            last_rescan: now,
        }
    }

    pub fn changed(&mut self, source: i64, now: Instant) {
        self.changes.insert(source, now);
    }

    /// Takes what is due at `now`; `rescan` is the periodic interval, if
    /// any.
    pub fn due(&mut self, now: Instant, rescan: Option<Duration>) -> Due {
        let mut due = Due::default();
        let mut wait: Option<Duration> = None;
        let mut soonest = |d: Duration| wait = Some(wait.map_or(d, |w| w.min(d)));
        self.changes.retain(|&id, &mut at| {
            let ready = at + SETTLE;
            if ready <= now {
                due.changed.push(id);
                false
            } else {
                soonest(ready - now);
                true
            }
        });
        if let Some(interval) = rescan {
            let next = self.last_rescan + interval;
            if next <= now {
                due.rescan = true;
                self.last_rescan = now;
                soonest(interval);
            } else {
                soonest(next - now);
            }
        }
        due.changed.sort_unstable();
        due.wait = wait;
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: Duration = Duration::from_secs(1);

    #[test]
    fn a_change_is_due_once_the_folder_is_quiet() {
        let t0 = Instant::now();
        let mut schedule = Schedule::new(t0);
        schedule.changed(7, t0);
        assert_eq!(schedule.due(t0 + S, None).wait, Some(S));
        // More changes keep pushing it back.
        schedule.changed(7, t0 + S);
        assert_eq!(schedule.due(t0 + 2 * S, None).changed, Vec::<i64>::new());
        let due = schedule.due(t0 + 3 * S, None);
        assert_eq!((due.changed, due.wait), (vec![7], None));
        assert_eq!(schedule.due(t0 + 9 * S, None), Due::default());
    }

    #[test]
    fn watched_sources_are_rescanned_periodically() {
        let t0 = Instant::now();
        let mut schedule = Schedule::new(t0);
        let every = Some(60 * S);
        assert_eq!(schedule.due(t0 + 10 * S, every).wait, Some(50 * S));
        let due = schedule.due(t0 + 61 * S, every);
        assert!(due.rescan);
        assert_eq!(due.wait, Some(60 * S));
        assert!(!schedule.due(t0 + 62 * S, every).rescan);
        // A shorter interval applies right away.
        assert!(schedule.due(t0 + 72 * S, Some(10 * S)).rescan);
        // Changes and the rescan: the sooner one sets the wait.
        schedule.changed(1, t0 + 72 * S);
        assert_eq!(schedule.due(t0 + 73 * S, every).wait, Some(S));
    }
}
