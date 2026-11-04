//! Locations of the files that belong to the application.

use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

const APP_DIR: &str = "arch-setting";

/// Returns the application's configuration directory for the current user.
///
/// Returns `None` when neither `XDG_CONFIG_HOME` nor `HOME` gives a usable location.
#[must_use]
pub fn config_dir() -> Option<PathBuf> {
    resolve_config_dir(
        env::var_os("XDG_CONFIG_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
    )
}

/// Applies the XDG Base Directory rules: a value that is empty or not absolute is ignored.
fn resolve_config_dir(xdg_config_home: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    let base = absolute(xdg_config_home)
        .map(Path::to_path_buf)
        .or_else(|| absolute(home).map(|home| home.join(".config")))?;
    Some(base.join(APP_DIR))
}

fn absolute(value: Option<&OsStr>) -> Option<&Path> {
    value.map(Path::new).filter(|path| path.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(xdg: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
        resolve_config_dir(xdg.map(OsStr::new), home.map(OsStr::new))
    }

    #[test]
    fn uses_xdg_config_home_when_absolute() {
        assert_eq!(
            resolve(Some("/xdg"), Some("/home/user")),
            Some(PathBuf::from("/xdg/arch-setting"))
        );
    }

    #[test]
    fn falls_back_to_home_when_xdg_is_missing() {
        assert_eq!(
            resolve(None, Some("/home/user")),
            Some(PathBuf::from("/home/user/.config/arch-setting"))
        );
    }

    #[test]
    fn ignores_empty_xdg_config_home() {
        assert_eq!(
            resolve(Some(""), Some("/home/user")),
            Some(PathBuf::from("/home/user/.config/arch-setting"))
        );
    }

    #[test]
    fn ignores_relative_xdg_config_home() {
        assert_eq!(
            resolve(Some("relative/path"), Some("/home/user")),
            Some(PathBuf::from("/home/user/.config/arch-setting"))
        );
    }

    #[test]
    fn returns_none_without_a_usable_location() {
        assert_eq!(resolve(None, None), None);
        assert_eq!(resolve(Some("relative"), Some("also-relative")), None);
    }
}
