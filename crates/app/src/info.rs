//! Application identity, shown in the UI's About screen.

use serde::Serialize;
use ts_rs::TS;

/// Name and version of the running application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

/// Returns the application's name and version.
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "Picturesque".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_reports_name_and_crate_version() {
        let info = app_info();

        assert_eq!(info.name, "Picturesque");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }
}
