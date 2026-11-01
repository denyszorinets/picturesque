//! [`PowerSource`] from Linux's sysfs; elsewhere power is not known yet and
//! counts as mains.

use std::path::Path;

use picturesque_app::ports::power::PowerSource;

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemPower;

impl PowerSource for SystemPower {
    fn on_battery(&self) -> bool {
        on_battery(Path::new("/sys/class/power_supply"))
    }
}

/// On battery when a battery is discharging and no mains supply is online.
fn on_battery(supplies: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(supplies) else {
        return false;
    };
    let read = |dir: &Path, name: &str| {
        std::fs::read_to_string(dir.join(name))
            .map(|s| s.trim().to_owned())
            .unwrap_or_default()
    };
    let mut discharging = false;
    for entry in entries.flatten() {
        let dir = entry.path();
        match read(&dir, "type").as_str() {
            "Mains" if read(&dir, "online") == "1" => return false,
            "Battery" if read(&dir, "status") == "Discharging" => discharging = true,
            _ => {}
        }
    }
    discharging
}

#[cfg(test)]
mod tests {
    use super::*;

    fn supply(root: &Path, name: &str, files: &[(&str, &str)]) {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        for (file, value) in files {
            std::fs::write(dir.join(file), format!("{value}\n")).unwrap();
        }
    }

    #[test]
    fn reads_batteries_and_mains() {
        let root = tempfile::tempdir().unwrap();
        // A desktop: no supplies at all.
        assert!(!on_battery(root.path()));
        supply(
            root.path(),
            "BAT0",
            &[("type", "Battery"), ("status", "Discharging")],
        );
        supply(root.path(), "AC", &[("type", "Mains"), ("online", "0")]);
        assert!(on_battery(root.path()));
        supply(root.path(), "AC", &[("type", "Mains"), ("online", "1")]);
        assert!(!on_battery(root.path()));
        assert!(!on_battery(&root.path().join("missing")));
    }
}
