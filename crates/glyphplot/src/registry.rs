//! The chart families this build holds, in registry order: one in a family's
//! wasm module, every one with the feature `all`.

use crate::Budget;

/// A chart family: one Cargo feature and one wasm module, `web/pkg/<name>/`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Family {
    /// The feature's and the module's name.
    pub name: &'static str,
    /// What the family's module may cost.
    pub budget: Budget,
}

/// Every family compiled in.
pub const FAMILIES: &[Family] = &[
    #[cfg(feature = "line")]
    crate::charts::line::FAMILY,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// A module name as `spec/schemas/budgets.schema.json` allows it: lowercase
    /// words of letters and digits joined by single hyphens.
    fn is_module_name(name: &str) -> bool {
        !name.is_empty() && name.split('-').all(|word| !word.is_empty() && word.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
    }

    #[test]
    fn names_are_module_names_and_unique() {
        for (i, family) in FAMILIES.iter().enumerate() {
            assert!(is_module_name(family.name), "{:?} is not a module name", family.name);
            assert!(FAMILIES[..i].iter().all(|f| f.name != family.name), "{:?} is registered twice", family.name);
        }
    }

    #[test]
    fn module_names() {
        assert!(is_module_name("line") && is_module_name("sky-2"));
        assert!(!is_module_name("") && !is_module_name("Line") && !is_module_name("a--b") && !is_module_name("-a") && !is_module_name("a_b"));
    }

    #[cfg(feature = "all")]
    #[test]
    fn all_holds_every_family() {
        let names: Vec<&str> = FAMILIES.iter().map(|f| f.name).collect();
        assert_eq!(names, ["line"]);
    }
}
