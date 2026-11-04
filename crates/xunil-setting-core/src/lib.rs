//! UI-independent logic for xunil-setting.
//!
//! Nothing in this crate may depend on GTK, so it can be tested without a window.

pub mod paths;

/// Temporary code that must be rejected by CI.
#[must_use]
pub fn untested_badly_formatted(value: i32) -> i32 {
    if value > 0 { value * 2 } else { value - 1 }
}
