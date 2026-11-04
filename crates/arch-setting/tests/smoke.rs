//! Opens the real window on a display server without a screen.
//!
//! Run it through `scripts/ui-tests.sh`, which starts that server. It is ignored by default, and
//! it refuses to run without `ARCH_SETTING_HEADLESS`, so it never opens a window in a real session.

use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(30);

#[test]
#[ignore = "needs a headless display server: run scripts/ui-tests.sh"]
fn main_window_opens_without_errors() {
    assert!(
        std::env::var_os("ARCH_SETTING_HEADLESS").is_some(),
        "run this test through scripts/ui-tests.sh"
    );

    let home = tempfile::tempdir().expect("temporary HOME");
    let binary = std::env::var_os("ARCH_SETTING_TEST_BINARY")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_arch-setting").into());

    let mut child = Command::new(binary)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
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
        .env("ARCH_SETTING_SMOKE_TEST", "1")
        .stdout(Stdio::null())
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
}
