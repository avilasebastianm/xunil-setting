//! Opens the real window on a display server without a screen.
//!
//! Run it through `scripts/ui-tests.sh`, which starts that server. It is ignored by default, and
//! it refuses to run without `XUNIL_SETTING_HEADLESS`, so it never opens a window in a real session.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(30);
const HYPRLAND_VAR: &str = "HYPRLAND_INSTANCE_SIGNATURE";

/// Runs the application until it exits by itself, and returns what it printed to stdout.
///
/// `PATH` only contains `bin`, so what the test finds does not depend on the machine. The
/// application only looks programs up, so nothing in `bin` ever runs.
fn run_application(bin: &Path, extra_env: &[(&str, &str)]) -> String {
    assert!(
        std::env::var_os("XUNIL_SETTING_HEADLESS").is_some(),
        "run this test through scripts/ui-tests.sh"
    );

    let home = tempfile::tempdir().expect("temporary HOME");
    let binary = std::env::var_os("XUNIL_SETTING_TEST_BINARY")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_xunil-setting").into());

    let mut child = Command::new(binary)
        .env_clear()
        .env("PATH", bin)
        .env("DISPLAY", std::env::var_os("DISPLAY").unwrap_or_default())
        .env("GDK_BACKEND", "x11")
        .env("GSETTINGS_BACKEND", "memory")
        .env("GTK_A11Y", "none")
        .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent")
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join("config"))
        .env("XDG_DATA_HOME", home.path().join("data"))
        .env("XDG_CACHE_HOME", home.path().join("cache"))
        .env("XDG_STATE_HOME", home.path().join("state"))
        .env("XUNIL_SETTING_SMOKE_TEST", "1")
        .envs(extra_env.iter().copied())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("application starts");

    let started = Instant::now();
    while child.try_wait().expect("application status").is_none() {
        if started.elapsed() > TIMEOUT {
            child.kill().expect("stop the application");
            child.wait().expect("reap the application");
            panic!("the application did not exit within {TIMEOUT:?}");
        }
        sleep(Duration::from_millis(50));
    }

    let output = child.wait_with_output().expect("application output");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit status {}: {stderr}",
        output.status
    );
    for marker in ["CRITICAL", "WARNING", "ERROR"] {
        assert!(!stderr.contains(marker), "{marker} in stderr: {stderr}");
    }
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn create_executable(dir: &Path, name: &str) {
    let path = dir.join(name);
    fs::write(&path, "").expect("create the file");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("set the mode");
}

#[test]
#[ignore = "needs a headless display server: run scripts/ui-tests.sh"]
fn opens_without_panels_on_a_bare_system() {
    let bin = tempfile::tempdir().expect("temporary directory");

    let stdout = run_application(bin.path(), &[]);

    assert!(!stdout.contains("panel:"), "unexpected panels: {stdout}");
}

#[test]
#[ignore = "needs a headless display server: run scripts/ui-tests.sh"]
fn hides_the_hyprland_panel_when_a_requirement_is_missing() {
    let bin = tempfile::tempdir().expect("temporary directory");

    let stdout = run_application(bin.path(), &[(HYPRLAND_VAR, "test")]);

    assert!(!stdout.contains("panel:"), "unexpected panels: {stdout}");
}

#[test]
#[ignore = "needs a headless display server: run scripts/ui-tests.sh"]
fn shows_the_hyprland_panel_when_it_is_detected() {
    let bin = tempfile::tempdir().expect("temporary directory");
    create_executable(bin.path(), "hyprctl");

    let stdout = run_application(bin.path(), &[(HYPRLAND_VAR, "test")]);

    assert!(stdout.contains("panel: hyprland"), "panels: {stdout}");
}
