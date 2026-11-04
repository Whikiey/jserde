//! `JavaSerializable`: explicit Rust ↔ Java class binding.
//!
//! The derive macro in `jser-derive` generates this trait plus the typed
//! `FromJValue`/`IntoJValue` implementations. The trait is intentionally
//! independent of serde: Java needs class names, type codes and SUIDs.

use crate::error::Result;
use crate::value::{FieldType, Graph, JValue};

/// One serializable field in declaration (Java) order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaField {
    /// Java field name.
    pub name: &'static str,
    /// Java type code.
    pub typecode: FieldType,
    /// JVM descriptor for `L`/`[` fields.
    pub class_name1: Option<&'static str>,
}

/// A Rust type bound to a Java `Serializable` class.
pub trait JavaSerializable: Sized {
    /// Dot-separated Java class name (`com.example.Foo`).
    const CLASS: &'static str;

    /// `serialVersionUID` written into the class descriptor.
    const SUID: i64;

    /// JVM field descriptor for this class (`Lcom/example/Foo;`).
    ///
    /// Used when another `JavaSerializable` type has a field of this type; the
    /// derive macro emits it from the `class` attribute.
    const DESCRIPTOR: &'static str;

    /// Fields in Java wire order: primitives by name, then objects by name.
    fn schema() -> &'static [JavaField];

    /// Convert to a Java object value, allocating in `graph`.
    fn to_jvalue(&self, graph: &mut Graph) -> Result<JValue>;

    /// Convert from a decoded Java object value.
    fn from_jvalue(value: &JValue, graph: &Graph) -> Result<Self>;
}
