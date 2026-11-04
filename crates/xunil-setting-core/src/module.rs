//! Modules, the panels they provide, and the registry that decides which ones apply.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use crate::capability::{Capability, Environment};

/// A page of the settings window.
#[derive(Debug, PartialEq, Eq)]
pub struct PanelDescriptor {
    /// Stable identifier, unique among all panels.
    pub id: &'static str,
    /// Title in English. The interface translates it.
    pub title: &'static str,
    /// Name of the icon in the icon theme.
    pub icon_name: &'static str,
}

/// A group of panels that share the same requirements.
#[derive(Debug)]
pub struct ModuleDescriptor {
    /// Stable identifier, unique among all modules.
    pub id: &'static str,
    /// Everything the system must offer for the module to be active.
    pub requires: &'static [Capability],
    /// The panels shown while the module is active.
    pub panels: &'static [PanelDescriptor],
}

impl ModuleDescriptor {
    /// Returns the requirements that the system does not offer.
    #[must_use]
    pub fn missing(&self, environment: &dyn Environment) -> Vec<Capability> {
        self.requires
            .iter()
            .copied()
            .filter(|capability| !environment.provides(*capability))
            .collect()
    }
}

/// The identifier that two modules or two panels have in common.
#[derive(Debug, PartialEq, Eq)]
pub struct DuplicateId(pub &'static str);

impl fmt::Display for DuplicateId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "duplicate identifier: {}", self.0)
    }
}

impl Error for DuplicateId {}

/// All the modules the application knows about.
#[derive(Debug)]
pub struct Registry {
    modules: Vec<&'static ModuleDescriptor>,
}

impl Registry {
    /// Creates a registry from a list of modules.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateId`] when two modules or two panels share an identifier.
    pub fn new(modules: Vec<&'static ModuleDescriptor>) -> Result<Self, DuplicateId> {
        let mut module_ids = HashSet::new();
        let mut panel_ids = HashSet::new();
        for module in &modules {
            if !module_ids.insert(module.id) {
                return Err(DuplicateId(module.id));
            }
            for panel in module.panels {
                if !panel_ids.insert(panel.id) {
                    return Err(DuplicateId(panel.id));
                }
            }
        }
        Ok(Self { modules })
    }

    /// Returns the panels of the modules whose requirements are all met, in registry order.
    #[must_use]
    pub fn available_panels(&self, environment: &dyn Environment) -> Vec<&'static PanelDescriptor> {
        self.modules
            .iter()
            .filter(|module| module.missing(environment).is_empty())
            .flat_map(|module| module.panels)
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::HashSet;

    use super::*;

    /// A system that offers exactly the capabilities a test gives it.
    #[derive(Default)]
    pub(crate) struct FakeEnvironment {
        pub(crate) binaries: HashSet<&'static str>,
        pub(crate) env_vars: HashSet<&'static str>,
    }

    impl Environment for FakeEnvironment {
        fn has_binary(&self, name: &str) -> bool {
            self.binaries.contains(name)
        }

        fn has_env_var(&self, name: &str) -> bool {
            self.env_vars.contains(name)
        }
    }

    static PANEL: PanelDescriptor = PanelDescriptor {
        id: "panel",
        title: "Panel",
        icon_name: "icon",
    };
    static ALWAYS: ModuleDescriptor = ModuleDescriptor {
        id: "always",
        requires: &[],
        panels: std::slice::from_ref(&PANEL),
    };
    static SAME_ID: ModuleDescriptor = ModuleDescriptor {
        id: "always",
        requires: &[],
        panels: &[],
    };
    static SAME_PANEL: ModuleDescriptor = ModuleDescriptor {
        id: "other",
        requires: &[],
        panels: std::slice::from_ref(&PANEL),
    };
    static NEEDS_BOTH: ModuleDescriptor = ModuleDescriptor {
        id: "needs-both",
        requires: &[Capability::Binary("tool"), Capability::EnvVar("VARIABLE")],
        panels: &[PanelDescriptor {
            id: "needs-both-panel",
            title: "Needs both",
            icon_name: "icon",
        }],
    };

    #[test]
    fn module_without_requirements_is_always_available() {
        let registry = Registry::new(vec![&ALWAYS]).expect("unique identifiers");

        let panels = registry.available_panels(&FakeEnvironment::default());

        assert_eq!(panels, [&PANEL]);
    }

    #[test]
    fn module_appears_only_when_every_requirement_is_met() {
        let registry = Registry::new(vec![&NEEDS_BOTH]).expect("unique identifiers");
        let mut environment = FakeEnvironment::default();
        assert_eq!(
            registry.available_panels(&environment),
            [] as [&PanelDescriptor; 0]
        );

        environment.binaries.insert("tool");
        assert_eq!(
            registry.available_panels(&environment),
            [] as [&PanelDescriptor; 0]
        );

        environment.env_vars.insert("VARIABLE");
        assert_eq!(registry.available_panels(&environment).len(), 1);
    }

    #[test]
    fn reports_only_the_requirements_that_are_missing() {
        let environment = FakeEnvironment {
            binaries: HashSet::from(["tool"]),
            ..FakeEnvironment::default()
        };

        assert_eq!(
            NEEDS_BOTH.missing(&environment),
            [Capability::EnvVar("VARIABLE")]
        );
    }

    #[test]
    fn rejects_duplicate_module_ids() {
        let error = Registry::new(vec![&ALWAYS, &SAME_ID]).expect_err("duplicate module");

        assert_eq!(error, DuplicateId("always"));
        assert_eq!(error.to_string(), "duplicate identifier: always");
    }

    #[test]
    fn rejects_duplicate_panel_ids() {
        let error = Registry::new(vec![&ALWAYS, &SAME_PANEL]).expect_err("duplicate panel");

        assert_eq!(error, DuplicateId("panel"));
    }
}
