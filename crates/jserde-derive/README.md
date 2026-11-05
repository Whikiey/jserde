# jserde-derive

`#[derive(JavaSerializable)]` for the [jserde](../jserde) core crate.

```rust
use jserde_derive::JavaSerializable;

#[derive(JavaSerializable)]
#[java(class = "com.example.Point", suid = 1)]
struct Point {
    x: i32,
    y: i32,
}
```

See the workspace [README](../../README.md) and [guide](../../AGENT.md) for
supported field types, generics, inheritance, and release status.

License: MIT OR Apache-2.0.
