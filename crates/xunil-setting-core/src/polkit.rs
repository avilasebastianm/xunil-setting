//! The answers of system services that ask polkit for permission.
//!
//! The application never asks for a password. A service does it through polkit, and its D-Bus error
//! name says what happened. See `docs/adr/0008-privileges-through-system-services.md`.

const ACCESS_DENIED: &str = "org.freedesktop.DBus.Error.AccessDenied";
const INTERACTIVE_AUTHORIZATION_REQUIRED: &str =
    "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired";
const POLKIT_NOT_AUTHORIZED: &str = "org.freedesktop.PolicyKit1.Error.NotAuthorized";
const POLKIT_CANCELLED: &str = "org.freedesktop.PolicyKit1.Error.Cancelled";
const NETWORK_MANAGER_PREFIX: &str = "org.freedesktop.NetworkManager.";
const NETWORK_MANAGER_DENIED_SUFFIX: &str = ".PermissionDenied";

/// The reason a system service did not get permission to make a change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorizationFailure {
    /// Permission was refused. Some services also answer this when the password dialog is closed.
    Denied,
    /// The request was cancelled before polkit answered.
    Cancelled,
    /// A password is needed, but no polkit agent is running to ask for it.
    ///
    /// This only holds for calls that allow interactive authorization. Without that D-Bus flag,
    /// services give the same answer even when an agent is running.
    NoAgent,
}

impl AuthorizationFailure {
    /// Returns the failure behind a D-Bus error name, or `None` if the error is about something else.
    #[must_use]
    pub fn from_dbus_error(name: &str) -> Option<Self> {
        match name {
            ACCESS_DENIED | POLKIT_NOT_AUTHORIZED => Some(Self::Denied),
            POLKIT_CANCELLED => Some(Self::Cancelled),
            INTERACTIVE_AUTHORIZATION_REQUIRED => Some(Self::NoAgent),
            // NetworkManager has one of these for each of its interfaces.
            _ if name.starts_with(NETWORK_MANAGER_PREFIX)
                && name.ends_with(NETWORK_MANAGER_DENIED_SUFFIX) =>
            {
                Some(Self::Denied)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_a_denied_request() {
        for name in [
            "org.freedesktop.DBus.Error.AccessDenied",
            "org.freedesktop.PolicyKit1.Error.NotAuthorized",
        ] {
            assert_eq!(
                AuthorizationFailure::from_dbus_error(name),
                Some(AuthorizationFailure::Denied),
                "{name}"
            );
        }
    }

    #[test]
    fn recognizes_every_network_manager_denial() {
        for name in [
            "org.freedesktop.NetworkManager.PermissionDenied",
            "org.freedesktop.NetworkManager.Settings.PermissionDenied",
            "org.freedesktop.NetworkManager.AgentManager.PermissionDenied",
        ] {
            assert_eq!(
                AuthorizationFailure::from_dbus_error(name),
                Some(AuthorizationFailure::Denied),
                "{name}"
            );
        }
    }

    #[test]
    fn recognizes_a_cancelled_request() {
        assert_eq!(
            AuthorizationFailure::from_dbus_error("org.freedesktop.PolicyKit1.Error.Cancelled"),
            Some(AuthorizationFailure::Cancelled)
        );
    }

    #[test]
    fn recognizes_a_missing_agent() {
        assert_eq!(
            AuthorizationFailure::from_dbus_error(
                "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired"
            ),
            Some(AuthorizationFailure::NoAgent)
        );
    }

    #[test]
    fn leaves_other_errors_alone() {
        for name in [
            "",
            "org.freedesktop.DBus.Error.ServiceUnknown",
            "org.freedesktop.PolicyKit1.Error.Failed",
            "org.freedesktop.NetworkManager.UnknownConnection",
            "org.example.PermissionDenied",
            // BlueZ answers this about a paired device, not about polkit.
            "org.bluez.Error.NotAuthorized",
        ] {
            assert_eq!(AuthorizationFailure::from_dbus_error(name), None, "{name}");
        }
    }
}
