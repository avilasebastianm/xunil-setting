//! Detection of what the current system offers.

use std::env;
use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Something a module needs from the system in order to work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    /// An executable with this name in `PATH`. It is only looked up, never run.
    Binary(&'static str),
    /// An environment variable with this name, such as the one a compositor sets for its session.
    EnvVar(&'static str),
}

/// Answers questions about the system, so tests can replace the real one.
pub trait Environment {
    /// Tells whether an executable with this name is in `PATH`.
    fn has_binary(&self, name: &str) -> bool;

    /// Tells whether this environment variable is set.
    fn has_env_var(&self, name: &str) -> bool;

    /// Tells whether the system offers this capability.
    fn provides(&self, capability: Capability) -> bool {
        match capability {
            Capability::Binary(name) => self.has_binary(name),
            Capability::EnvVar(name) => self.has_env_var(name),
        }
    }
}

/// The system the application is running on.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemEnvironment;

impl Environment for SystemEnvironment {
    fn has_binary(&self, name: &str) -> bool {
        binary_in_path(env::var_os("PATH").as_deref(), name)
    }

    fn has_env_var(&self, name: &str) -> bool {
        env::var_os(name).is_some()
    }
}

/// Relative entries in `PATH` are ignored, because they depend on the current directory.
fn binary_in_path(path: Option<&OsStr>, name: &str) -> bool {
    path.is_some_and(|path| {
        env::split_paths(path)
            .filter(|dir| dir.is_absolute())
            .any(|dir| is_executable(&dir.join(name)))
    })
}

fn is_executable(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    fn create_file(dir: &Path, name: &str, mode: u32) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, "").expect("create the file");
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).expect("set the mode");
        path
    }

    #[test]
    fn finds_an_executable_in_any_path_entry() {
        let empty = tempfile::tempdir().expect("temporary directory");
        let bin = tempfile::tempdir().expect("temporary directory");
        create_file(bin.path(), "tool", 0o755);
        let path = env::join_paths([empty.path(), bin.path()]).expect("join the paths");

        assert!(binary_in_path(Some(&path), "tool"));
    }

    #[test]
    fn ignores_a_file_that_is_not_executable() {
        let bin = tempfile::tempdir().expect("temporary directory");
        create_file(bin.path(), "tool", 0o644);

        assert!(!binary_in_path(Some(bin.path().as_os_str()), "tool"));
    }

    #[test]
    fn ignores_a_directory_with_the_same_name() {
        let bin = tempfile::tempdir().expect("temporary directory");
        fs::create_dir(bin.path().join("tool")).expect("create the directory");

        assert!(!binary_in_path(Some(bin.path().as_os_str()), "tool"));
    }

    #[test]
    fn ignores_relative_and_empty_path_entries() {
        assert!(!binary_in_path(Some(OsStr::new("")), "Cargo.toml"));
        assert!(!binary_in_path(Some(OsStr::new(".")), "Cargo.toml"));
    }

    #[test]
    fn finds_nothing_without_a_path() {
        assert!(!binary_in_path(None, "tool"));
    }

    #[test]
    fn system_environment_reports_what_is_missing() {
        let system = SystemEnvironment;

        assert!(!system.provides(Capability::Binary("xunil-setting-no-such-binary")));
        assert!(!system.provides(Capability::EnvVar("XUNIL_SETTING_NO_SUCH_VARIABLE")));
    }
}
