//! Application identity, shown in the UI's About screen.

/// Name and version of the running application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
}

/// Returns the application's name and version.
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "Picturesque",
        version: env!("CARGO_PKG_VERSION"),
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
