# jserde-serde

Optional schema-driven Serde bridge for [jserde](../jserde). The core crate
does not depend on Serde.

```rust
let bytes = jserde_serde::to_vec(&value, &schema)?;
let value = jserde_serde::from_slice(&bytes, &schema, &allowlist)?;
```

Supports nested POJOs, Map/Set/enum fields and roots, multi-root/`TC_RESET`
streams, and non-cyclic `Shared<T>` identity. Full object cycles use the
dynamic `jserde::Graph` API.

See the workspace [README](../../README.md) for details and limitations.

License: MIT OR Apache-2.0.
