//! Class allowlist for deserialization.
//!
//! This is a **defense-in-depth hook**, not a gadget sandbox: an allowed class
//! can still contain dangerous `readObject` behavior. It only prevents newly
//! encountered classes from being instantiated without an explicit policy grant.

use std::collections::HashSet;

use crate::error::{Error, Result};

/// Class-name policy applied while parsing each `TC_CLASSDESC`.
#[derive(Debug, Clone, Default)]
pub struct Allowlist {
    all: bool,
    exact: HashSet<String>,
    prefixes: Vec<String>,
}

impl Allowlist {
    /// Deny every class unless explicitly added.
    pub fn new() -> Self {
        Self::default()
    }

    /// Default policy: `java.lang.*` and `java.util.*`.
    pub fn strict() -> Self {
        let mut allowlist = Self::new();
        allowlist
            .add("java.lang.*")
            .expect("built-in allowlist pattern is valid");
        allowlist
            .add("java.util.*")
            .expect("built-in allowlist pattern is valid");
        allowlist
    }

    /// Allow every class. Intended for tests and explicitly trusted streams.
    pub fn all() -> Self {
        let mut allowlist = Self::new();
        allowlist.all = true;
        allowlist
    }

    /// Add an exact class name, a `prefix.*` pattern, or `Fixtures$*`-style
    /// prefix ending in `$*`. A bare `*` allows everything.
    pub fn add(&mut self, pattern: &str) -> Result<()> {
        if pattern.is_empty() {
            return Err(Error::InvalidSchema(
                "allowlist pattern must not be empty".to_owned(),
            ));
        }
        if pattern == "*" {
            self.all = true;
            return Ok(());
        }

        if let Some(prefix) = pattern.strip_suffix('*') {
            if prefix.is_empty()
                || !(prefix.ends_with('.') || prefix.ends_with('$'))
                || prefix[..prefix.len() - 1].contains('*')
            {
                return Err(Error::InvalidSchema(format!(
                    "invalid allowlist prefix pattern `{pattern}`"
                )));
            }
            self.prefixes.push(prefix.to_owned());
            return Ok(());
        }

        if pattern.contains('*') {
            return Err(Error::InvalidSchema(format!(
                "invalid allowlist pattern `{pattern}`"
            )));
        }

        self.exact.insert(pattern.to_owned());
        Ok(())
    }

    /// Builder form of [`Allowlist::add`].
    pub fn allow(mut self, pattern: &str) -> Result<Self> {
        self.add(pattern)?;
        Ok(self)
    }

    /// Builder form of [`Allowlist::all`].
    pub fn allow_all(mut self) -> Self {
        self.all = true;
        self
    }

    /// Whether the policy is "allow everything".
    pub fn allows_all(&self) -> bool {
        self.all
    }

    /// Check one class name (including array class names).
    pub fn is_allowed(&self, class: &str) -> bool {
        if self.all {
            return true;
        }
        if class.starts_with('[') {
            return self.is_array_allowed(class);
        }

        self.exact.contains(class) || self.prefixes.iter().any(|prefix| class.starts_with(prefix))
    }

    /// Return [`Error::UnsupportedClass`] when `class` is not allowed.
    pub fn check(&self, class: &str) -> Result<()> {
        if self.is_allowed(class) {
            Ok(())
        } else {
            Err(Error::UnsupportedClass(class.to_owned()))
        }
    }

    fn is_array_allowed(&self, class: &str) -> bool {
        let mut rest = class;
        while let Some(stripped) = rest.strip_prefix('[') {
            rest = stripped;
        }

        if rest.len() == 1 && b"BCDFIJSZ".contains(&rest.as_bytes()[0]) {
            return true;
        }

        if let Some(element) = rest
            .strip_prefix('L')
            .and_then(|element| element.strip_suffix(';'))
        {
            let element = element.replace('/', ".");
            return self.is_allowed(&element);
        }

        false
    }
}
