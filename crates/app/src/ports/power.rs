//! Whether the computer runs on battery, so gentle background work can
//! wait for mains power (`critic::background`).

pub trait PowerSource: Send + Sync {
    /// `true` only when known to run on battery.
    fn on_battery(&self) -> bool;
}

/// Always on mains power (desktops, and systems where it is not known).
#[derive(Debug, Default, Clone, Copy)]
pub struct MainsPower;

impl PowerSource for MainsPower {
    fn on_battery(&self) -> bool {
        false
    }
}
