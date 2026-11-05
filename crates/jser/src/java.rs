//! `JavaSerializable`: explicit Rust ↔ Java class binding.
//!
//! The derive macro in `jser-derive` generates this trait plus the typed
//! `FromJValue`/`IntoJValue` implementations. The trait is intentionally
//! independent of serde: Java needs class names, type codes and SUIDs.

use crate::error::{Error, Result};
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

    /// JVM array descriptor for arrays of this class (`[Lcom/example/Foo;`).
    ///
    /// Used by `Vec<Custom>` fields; the derive macro emits it from `class`.
    const ARRAY_DESCRIPTOR: &'static str;

    /// Fields in Java wire order: primitives by name, then objects by name.
    ///
    /// This returns an owned vector so a generic `JavaSerializable` can expose
    /// field descriptors that depend on its monomorphized type parameters.
    fn schema() -> Vec<JavaField>;

    /// Convert to a Java object value, allocating in `graph`.
    fn to_jvalue(&self, graph: &mut Graph) -> Result<JValue>;

    /// Convert from a decoded Java object value.
    fn from_jvalue(value: &JValue, graph: &Graph) -> Result<Self>;

    /// Check that `value` is an object (or enum) of this Java class and SUID.
    ///
    /// A subclass stream is accepted when its class chain contains a layer with
    /// `Self::CLASS` and the matching SUID; this is what Java-compatible
    /// superclass decoding and `#[java(flatten)]` need. A class with the same
    /// fields but a different name, or a matching name with a different SUID,
    /// returns [`Error::FieldMismatch`] with both SUIDs in the message.
    fn validate_decoded(value: &JValue, graph: &Graph) -> Result<()> {
        let expected = Self::CLASS;
        let expected_suid = Self::SUID;
        let actual_name = graph.class_name(value).unwrap_or("<non-reference>");
        let actual_suid = graph
            .descriptor_of(value)
            .map(|desc| desc.suid)
            .unwrap_or_default();

        if let Some(object) = graph.as_object(value)? {
            for class in &object.classes {
                let desc = graph.descriptor(class.desc)?;
                if desc.name == expected {
                    if desc.suid == expected_suid {
                        return Ok(());
                    }
                    return Err(Error::FieldMismatch {
                        message: format!(
                            "serialVersionUID mismatch for `{expected}`: expected {expected_suid}, got {}",
                            desc.suid
                        ),
                    });
                }
            }
        } else if actual_name == expected {
            if actual_suid == expected_suid {
                return Ok(());
            }
            return Err(Error::FieldMismatch {
                message: format!(
                    "serialVersionUID mismatch for `{expected}`: expected {expected_suid}, got {actual_suid}"
                ),
            });
        }

        Err(Error::FieldMismatch {
            message: format!(
                "class name mismatch: expected `{expected}` (SUID {expected_suid}), got `{actual_name}` (SUID {actual_suid})"
            ),
        })
    }
}
