//! Typed conversions between Rust values and the dynamic [`Graph`] model.
//!
//! `decode_typed`/`encode_typed` are the convenience layer for known POJOs and
//! collections. The dynamic graph remains the source of truth: unknown classes
//! and metadata are preserved by the decoder/encoder, not by these traits.
//!
//! Collections are encoded as their Java collection classes (`ArrayList`,
//! `HashMap`, `HashSet`); primitive elements are boxed into `java.lang.*`
//! wrappers before being placed in a collection, as Java requires.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::builtin::{new_array_list, new_hash_map_entries, new_hash_set};
use crate::error::{Error, Result};
use crate::value::{Graph, JValue};

/// Convert a decoded [`JValue`] into a Rust value.
pub trait FromJValue: Sized {
    /// Convert `value` using `graph` for reference resolution.
    fn from_jvalue(value: &JValue, graph: &Graph) -> Result<Self>;
}

/// Convert a Rust value into a [`JValue`], allocating reference values in `graph`.
pub trait IntoJValue: Sized {
    /// Convert `self`, using `graph` for string/object allocation.
    fn into_jvalue(self, graph: &mut Graph) -> Result<JValue>;
}

/// Decode a single-element stream and convert it to `T`.
pub fn decode_typed<T: FromJValue>(bytes: &[u8]) -> Result<T> {
    let graph = crate::decode(bytes)?;
    let value = &graph.items()[0];
    T::from_jvalue(value, &graph)
}

/// Decode a single-element stream with an explicit class allowlist.
pub fn decode_typed_with_allowlist<T: FromJValue>(
    bytes: &[u8],
    allowlist: &crate::Allowlist,
) -> Result<T> {
    let graph = crate::decode_with_allowlist(bytes, allowlist)?;
    let value = &graph.items()[0];
    T::from_jvalue(value, &graph)
}

/// Convert `value` to a single-element stream.
pub fn encode_typed<T: IntoJValue>(value: T) -> Result<Vec<u8>> {
    let mut graph = Graph::new();
    let value = value.into_jvalue(&mut graph)?;
    graph.add_item(value);
    crate::encode_stream(&graph)
}

fn mismatch(message: impl Into<String>) -> Error {
    Error::FieldMismatch {
        message: message.into(),
    }
}

impl FromJValue for JValue {
    fn from_jvalue(value: &JValue, _graph: &Graph) -> Result<Self> {
        Ok(value.clone())
    }
}

impl IntoJValue for JValue {
    fn into_jvalue(self, _graph: &mut Graph) -> Result<JValue> {
        Ok(self)
    }
}

impl FromJValue for String {
    fn from_jvalue(value: &JValue, graph: &Graph) -> Result<Self> {
        graph.as_str(value).map(str::to_owned)
    }
}

impl IntoJValue for String {
    fn into_jvalue(self, graph: &mut Graph) -> Result<JValue> {
        Ok(JValue::Str(graph.new_string(self)))
    }
}

impl IntoJValue for &str {
    fn into_jvalue(self, graph: &mut Graph) -> Result<JValue> {
        Ok(JValue::Str(graph.new_string(self)))
    }
}

impl FromJValue for bool {
    fn from_jvalue(value: &JValue, graph: &Graph) -> Result<Self> {
        graph.as_bool(value)
    }
}

impl IntoJValue for bool {
    fn into_jvalue(self, _graph: &mut Graph) -> Result<JValue> {
        Ok(JValue::Bool(self))
    }
}

impl FromJValue for char {
    fn from_jvalue(value: &JValue, graph: &Graph) -> Result<Self> {
        let unit = graph.as_char(value)?;
        char::from_u32(u32::from(unit)).ok_or_else(|| mismatch("unpaired UTF-16 surrogate in char"))
    }
}

impl IntoJValue for char {
    fn into_jvalue(self, _graph: &mut Graph) -> Result<JValue> {
        Ok(JValue::Char(self as u16))
    }
}

impl FromJValue for u16 {
    fn from_jvalue(value: &JValue, graph: &Graph) -> Result<Self> {
        graph.as_char(value)
    }
}

impl IntoJValue for u16 {
    fn into_jvalue(self, _graph: &mut Graph) -> Result<JValue> {
        Ok(JValue::Char(self))
    }
}

macro_rules! from_integer {
    ($($ty:ty),* $(,)?) => {
        $(
            impl FromJValue for $ty {
                fn from_jvalue(value: &JValue, graph: &Graph) -> Result<Self> {
                    let number = graph.as_i64(value)?;
                    <$ty>::try_from(number).map_err(|_| {
                        mismatch(format!(
                            "integer {number} does not fit in {}",
                            std::any::type_name::<$ty>()
                        ))
                    })
                }
            }
        )*
    };
}

from_integer!(i8, i16, i32, i64);

macro_rules! into_integer {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(
            impl IntoJValue for $ty {
                fn into_jvalue(self, _graph: &mut Graph) -> Result<JValue> {
                    Ok(JValue::$variant(self))
                }
            }
        )*
    };
}

into_integer!(
    i8 => Byte,
    i16 => Short,
    i32 => Int,
    i64 => Long,
);

impl FromJValue for f32 {
    fn from_jvalue(value: &JValue, graph: &Graph) -> Result<Self> {
        Ok(graph.as_f64(value)? as f32)
    }
}

impl FromJValue for f64 {
    fn from_jvalue(value: &JValue, graph: &Graph) -> Result<Self> {
        graph.as_f64(value)
    }
}

impl IntoJValue for f32 {
    fn into_jvalue(self, _graph: &mut Graph) -> Result<JValue> {
        Ok(JValue::Float(self))
    }
}

impl IntoJValue for f64 {
    fn into_jvalue(self, _graph: &mut Graph) -> Result<JValue> {
        Ok(JValue::Double(self))
    }
}

impl<T: FromJValue> FromJValue for Option<T> {
    fn from_jvalue(value: &JValue, graph: &Graph) -> Result<Self> {
        match value {
            JValue::Null => Ok(None),
            other => T::from_jvalue(other, graph).map(Some),
        }
    }
}

impl<T: IntoJValue> IntoJValue for Option<T> {
    fn into_jvalue(self, graph: &mut Graph) -> Result<JValue> {
        match self {
            Some(value) => value.into_jvalue(graph),
            None => Ok(JValue::Null),
        }
    }
}

impl<T: FromJValue> FromJValue for Box<T> {
    fn from_jvalue(value: &JValue, graph: &Graph) -> Result<Self> {
        T::from_jvalue(value, graph).map(Box::new)
    }
}

impl<T: IntoJValue> IntoJValue for Box<T> {
    fn into_jvalue(self, graph: &mut Graph) -> Result<JValue> {
        (*self).into_jvalue(graph)
    }
}

impl<T: FromJValue> FromJValue for Vec<T> {
    fn from_jvalue(value: &JValue, graph: &Graph) -> Result<Self> {
        let values: Vec<&JValue> = if let Some(array) = graph.as_array(value)? {
            array.iter().collect()
        } else if let Some(list) = graph.as_list(value)? {
            list
        } else {
            return Err(mismatch(format!(
                "expected an array or List for {}, got {value:?}",
                std::any::type_name::<Vec<T>>()
            )));
        };

        values
            .into_iter()
            .map(|value| T::from_jvalue(value, graph))
            .collect()
    }
}

impl<T: IntoJValue> IntoJValue for Vec<T> {
    fn into_jvalue(self, graph: &mut Graph) -> Result<JValue> {
        let mut values = Vec::with_capacity(self.len());
        for value in self {
            values.push(value.into_jvalue(graph)?);
        }
        let handle = new_array_list(graph, values)?;
        Ok(JValue::Obj(handle))
    }
}

impl<K, V> FromJValue for HashMap<K, V>
where
    K: FromJValue + Eq + Hash,
    V: FromJValue,
{
    fn from_jvalue(value: &JValue, graph: &Graph) -> Result<Self> {
        let Some(view) = graph.as_map(value)? else {
            return Err(mismatch(format!(
                "expected a Map for {}, got {value:?}",
                std::any::type_name::<Self>()
            )));
        };

        let mut map = HashMap::with_capacity(view.entries.len());
        for (key, value) in view.entries {
            map.insert(K::from_jvalue(key, graph)?, V::from_jvalue(value, graph)?);
        }
        Ok(map)
    }
}

impl<K, V> IntoJValue for HashMap<K, V>
where
    K: IntoJValue + Eq + Hash,
    V: IntoJValue,
{
    fn into_jvalue(self, graph: &mut Graph) -> Result<JValue> {
        let mut entries = Vec::with_capacity(self.len());
        for (key, value) in self {
            entries.push((key.into_jvalue(graph)?, value.into_jvalue(graph)?));
        }
        let handle = new_hash_map_entries(graph, entries)?;
        Ok(JValue::Obj(handle))
    }
}

impl<T> FromJValue for HashSet<T>
where
    T: FromJValue + Eq + Hash,
{
    fn from_jvalue(value: &JValue, graph: &Graph) -> Result<Self> {
        let Some(values) = graph.as_set(value)? else {
            return Err(mismatch(format!(
                "expected a Set for {}, got {value:?}",
                std::any::type_name::<Self>()
            )));
        };

        let mut set = HashSet::with_capacity(values.len());
        for value in values {
            set.insert(T::from_jvalue(value, graph)?);
        }
        Ok(set)
    }
}

impl<T> IntoJValue for HashSet<T>
where
    T: IntoJValue + Eq + Hash,
{
    fn into_jvalue(self, graph: &mut Graph) -> Result<JValue> {
        let mut values = Vec::with_capacity(self.len());
        for value in self {
            values.push(value.into_jvalue(graph)?);
        }
        let handle = new_hash_set(graph, values)?;
        Ok(JValue::Obj(handle))
    }
}
