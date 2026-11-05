//! Opt-in pointer identity for the serde bridge.
//!
//! `Shared<T>` wraps an `Rc<T>` and preserves its identity within one
//! `to_vec`/`from_slice` call. Encoding writes one Java object and reuses the
//! same graph handle for later occurrences; decoding returns the same `Rc` for
//! repeated `TC_REFERENCE`s.
//!
//! This is deliberately a *partial* solution: serde is a tree model and cannot
//! represent a cycle without a two-phase `Rc<RefCell<...>>` construction. A
//! cycle is detected while decoding and reported as `FieldMismatch` instead of
//! recursing forever. Use the dynamic [`jser::Graph`] API for arbitrary cycles.

use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::rc::Rc;

use jser::{Handle, JValue};
use serde::de::{Deserialize, Deserializer, Error as DeError};
use serde::ser::{Serialize, Serializer};

thread_local! {
    static ENCODE_CACHE: RefCell<HashMap<usize, JValue>> = RefCell::new(HashMap::new());
    static ENCODE_PTRS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    static DECODE_CACHE: RefCell<HashMap<Handle, Rc<dyn Any>>> = RefCell::new(HashMap::new());
    static DECODE_IN_PROGRESS: RefCell<HashSet<Handle>> = RefCell::new(HashSet::new());
    static DECODE_HANDLES: RefCell<Vec<Handle>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn begin_encode() {
    ENCODE_CACHE.with(|cache| cache.borrow_mut().clear());
    ENCODE_PTRS.with(|stack| stack.borrow_mut().clear());
}

pub(crate) fn begin_decode() {
    DECODE_CACHE.with(|cache| cache.borrow_mut().clear());
    DECODE_IN_PROGRESS.with(|set| set.borrow_mut().clear());
    DECODE_HANDLES.with(|stack| stack.borrow_mut().clear());
}

pub(crate) fn current_pointer() -> Option<usize> {
    ENCODE_PTRS.with(|stack| stack.borrow().last().copied())
}

pub(crate) fn encoded(pointer: usize) -> Option<JValue> {
    ENCODE_CACHE.with(|cache| cache.borrow().get(&pointer).cloned())
}

pub(crate) fn remember_encoded(pointer: usize, value: JValue) {
    ENCODE_CACHE.with(|cache| {
        cache.borrow_mut().insert(pointer, value);
    });
}

struct PointerGuard(usize);

impl PointerGuard {
    fn new(pointer: usize) -> Self {
        ENCODE_PTRS.with(|stack| stack.borrow_mut().push(pointer));
        Self(pointer)
    }
}

impl Drop for PointerGuard {
    fn drop(&mut self) {
        ENCODE_PTRS.with(|stack| {
            let popped = stack.borrow_mut().pop();
            debug_assert_eq!(popped, Some(self.0));
        });
    }
}

pub(crate) fn current_handle() -> Option<Handle> {
    DECODE_HANDLES.with(|stack| stack.borrow().last().copied())
}

pub(crate) struct HandleGuard(Handle);

pub(crate) fn enter_handle(handle: Handle) -> HandleGuard {
    DECODE_HANDLES.with(|stack| stack.borrow_mut().push(handle));
    HandleGuard(handle)
}

impl Drop for HandleGuard {
    fn drop(&mut self) {
        DECODE_HANDLES.with(|stack| {
            let popped = stack.borrow_mut().pop();
            debug_assert_eq!(popped, Some(self.0));
        });
    }
}

pub(crate) fn handle_of(value: &JValue) -> Option<Handle> {
    match value {
        JValue::Str(handle)
        | JValue::Arr(handle)
        | JValue::Obj(handle)
        | JValue::Enum(handle)
        | JValue::Class(handle) => Some(*handle),
        _ => None,
    }
}

fn cached(handle: Handle) -> Option<Rc<dyn Any>> {
    DECODE_CACHE.with(|cache| cache.borrow().get(&handle).cloned())
}

fn mark_in_progress(handle: Handle) -> bool {
    DECODE_IN_PROGRESS.with(|set| set.borrow_mut().insert(handle))
}

fn unmark_in_progress(handle: Handle) {
    DECODE_IN_PROGRESS.with(|set| {
        set.borrow_mut().remove(&handle);
    });
}

fn remember_decoded(handle: Handle, value: Rc<dyn Any>) {
    DECODE_CACHE.with(|cache| {
        cache.borrow_mut().insert(handle, value);
    });
}

/// An `Rc<T>` with pointer identity preserved by the serde bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shared<T>(pub Rc<T>);

impl<T> Shared<T> {
    /// Wrap an existing `Rc`.
    pub fn new(value: Rc<T>) -> Self {
        Self(value)
    }

    /// Borrow the inner `Rc`.
    pub fn as_rc(&self) -> &Rc<T> {
        &self.0
    }
}

impl<T> Deref for Shared<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Serialize> Serialize for Shared<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let pointer = Rc::as_ptr(&self.0) as usize;
        let _guard = PointerGuard::new(pointer);
        serializer.serialize_newtype_struct("__jser_shared", &*self.0)
    }
}

impl<'de, T> Deserialize<'de> for Shared<T>
where
    T: Deserialize<'de> + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let handle = current_handle().ok_or_else(|| {
            D::Error::custom("Shared<T> is only supported inside the schema bridge")
        })?;
        if let Some(value) = cached(handle) {
            return match value.downcast::<T>() {
                Ok(value) => Ok(Shared(value)),
                Err(_) => Err(D::Error::custom(format!(
                    "shared handle {handle:?} was decoded with a different type"
                ))),
            };
        }
        if !mark_in_progress(handle) {
            return Err(D::Error::custom(format!(
                "cyclic shared reference at handle {handle:?}; use jser::Graph"
            )));
        }

        let result = T::deserialize(deserializer);
        unmark_in_progress(handle);
        let value = result.map_err(|_| D::Error::custom("failed to deserialize shared value"))?;
        let value = Rc::new(value);
        remember_decoded(handle, value.clone());
        Ok(Shared(value))
    }
}
