//! The modules that ship with the application.

use crate::capability::Capability;
use crate::module::{DuplicateId, ModuleDescriptor, PanelDescriptor, Registry};

static HYPRLAND: ModuleDescriptor = ModuleDescriptor {
    id: "hyprland",
    requires: &[
        Capability::EnvVar("HYPRLAND_INSTANCE_SIGNATURE"),
        Capability::Binary("hyprctl"),
    ],
    panels: &[PanelDescriptor {
        id: "hyprland",
        title: "Hyprland",
        icon_name: "video-display-symbolic",
    }],
};

/// Creates the registry with every module that ships with the application.
///
/// # Errors
///
/// Returns [`DuplicateId`] when two modules or two panels share an identifier.
pub fn builtin_registry() -> Result<Registry, DuplicateId> {
    Registry::new(vec![&HYPRLAND])
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::module::tests::FakeEnvironment;

    #[test]
    fn builtin_identifiers_are_unique() {
        assert!(builtin_registry().is_ok());
    }

    #[test]
    fn hyprland_is_hidden_outside_a_hyprland_session() {
        let registry = builtin_registry().expect("unique identifiers");
        let environment = FakeEnvironment {
            binaries: HashSet::from(["hyprctl"]),
            ..FakeEnvironment::default()
        };

        assert_eq!(
            registry.available_panels(&environment),
            [] as [&PanelDescriptor; 0]
        );
    }

    #[test]
    fn hyprland_appears_in_a_hyprland_session() {
        let registry = builtin_registry().expect("unique identifiers");
        let environment = FakeEnvironment {
            binaries: HashSet::from(["hyprctl"]),
            env_vars: HashSet::from(["HYPRLAND_INSTANCE_SIGNATURE"]),
        };

        let panels = registry.available_panels(&environment);

        assert_eq!(panels.len(), 1);
        assert_eq!(panels[0].id, "hyprland");
    }
}
