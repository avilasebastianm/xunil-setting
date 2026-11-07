//! Locations of the files that belong to the application.

use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

const APP_DIR: &str = "xunil-setting";

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

/// Returns the directory that keeps the backups of the files the application changes.
///
/// Returns `None` when neither `XDG_STATE_HOME` nor `HOME` gives a usable location.
#[must_use]
pub fn backup_dir() -> Option<PathBuf> {
    resolve_backup_dir(
        env::var_os("XDG_STATE_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
    )
}

fn resolve_config_dir(xdg_config_home: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    Some(resolve_base(xdg_config_home, home, ".config")?.join(APP_DIR))
}

fn resolve_backup_dir(xdg_state_home: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    Some(
        resolve_base(xdg_state_home, home, ".local/state")?
            .join(APP_DIR)
            .join("backups"),
    )
}

/// Applies the XDG Base Directory rules: a value that is empty or not absolute is ignored.
fn resolve_base(xdg: Option<&OsStr>, home: Option<&OsStr>, default: &str) -> Option<PathBuf> {
    absolute(xdg)
        .map(Path::to_path_buf)
        .or_else(|| absolute(home).map(|home| home.join(default)))
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
            Some(PathBuf::from("/xdg/xunil-setting"))
        );
    }

    #[test]
    fn falls_back_to_home_when_xdg_is_missing() {
        assert_eq!(
            resolve(None, Some("/home/user")),
            Some(PathBuf::from("/home/user/.config/xunil-setting"))
        );
    }

    #[test]
    fn ignores_empty_xdg_config_home() {
        assert_eq!(
            resolve(Some(""), Some("/home/user")),
            Some(PathBuf::from("/home/user/.config/xunil-setting"))
        );
    }

    #[test]
    fn ignores_relative_xdg_config_home() {
        assert_eq!(
            resolve(Some("relative/path"), Some("/home/user")),
            Some(PathBuf::from("/home/user/.config/xunil-setting"))
        );
    }

    #[test]
    fn returns_none_without_a_usable_location() {
        assert_eq!(resolve(None, None), None);
        assert_eq!(resolve(Some("relative"), Some("also-relative")), None);
    }

    fn resolve_backups(xdg: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
        resolve_backup_dir(xdg.map(OsStr::new), home.map(OsStr::new))
    }

    #[test]
    fn backups_use_xdg_state_home_when_absolute() {
        assert_eq!(
            resolve_backups(Some("/state"), Some("/home/user")),
            Some(PathBuf::from("/state/xunil-setting/backups"))
        );
    }

    #[test]
    fn backups_fall_back_to_home_when_xdg_is_unusable() {
        let expected = Some(PathBuf::from(
            "/home/user/.local/state/xunil-setting/backups",
        ));

        assert_eq!(resolve_backups(None, Some("/home/user")), expected);
        assert_eq!(resolve_backups(Some(""), Some("/home/user")), expected);
    }

    #[test]
    fn backups_return_none_without_a_usable_location() {
        assert_eq!(resolve_backups(None, None), None);
    }
}
