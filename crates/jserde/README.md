# jserde

Pure-Rust Java Object Serialization reader/writer, compatible with JDK
`ObjectOutputStream` / `ObjectInputStream`, with no runtime JVM.

```rust
let graph = jserde::decode_stream(&bytes)?;
```

See the workspace [README](../../README.md) for the API overview, compatibility
matrix, installation (after publication), security notes, and release status.

- Java 17 fixture baseline; wire format compatible with supported Java 1.2+ classes.
- Rust 1.77 MSRV; Linux/macOS/Windows CI.
- License: MIT OR Apache-2.0.
