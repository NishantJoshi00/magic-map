# Magic Map

A high-performance, thread-safe wrapper around mmap-sync providing seamless memory-mapped file operations in Rust. Perfect for applications requiring fast, concurrent access to persistent data.

## Description

Magic Map enhances memory-mapped file operations by providing:

- Thread-safe access to memory-mapped files
- Type-safe serialization and deserialization
- Efficient concurrent read/write operations
- Zero-copy data access where possible
- Automatic synchronization with persistent storage
- Built-in fallback mechanisms for error handling

Whether you're building a high-performance database, implementing a shared memory solution, or need efficient file-backed data structures, Magic Map provides a reliable and ergonomic API.

## Installation

Add Magic Map and its required dependencies to your `Cargo.toml`:

```toml
[dependencies]
magic-map = "0.1.0"
rkyv = { version = "0.7.40", features = ["validation", "strict"] }
bytecheck = { version = "~0.6.8", default-features = false }
```

### Dependencies Overview

- `rkyv`: Zero-copy serialization framework
- `bytecheck`: Data integrity validation
- `mmap-sync`: Low-level memory mapping operations

## Usage

### Basic Example

```rust
use magic_map::MagicMap;
use rkyv::{Archive, Serialize, Deserialize};
use bytecheck::CheckBytes;

#[derive(Archive, Debug, Serialize, Deserialize, Clone)]
#[archive_attr(derive(CheckBytes))]
struct User {
    id: u32,
    name: String,
}

fn main() {
    let mut map = MagicMap::new("users.bin".into());

    // Write data
    let user = User { id: 1, name: "Alice".into() };
    map.clone().create(user.clone()).unwrap();

    // Read data
    let loaded_user = map.get_owned().unwrap();
    assert_eq!(user.id, loaded_user.id);
}
```

### Advanced Usage

#### Concurrent Access

```rust
use std::thread;

let map = MagicMap::new("shared.bin".into());
let map_clone = map.clone();

let writer = thread::spawn(move || {
    map.write(|mut data| {
        // Modify data
        Ok(data)
    }, ()).unwrap();
});

let reader = thread::spawn(move || {
    map_clone.read(|data| {
        // Read data
        Ok(())
    }).unwrap();
});
```

#### Error Handling with Fallbacks

```rust
map.write(
    |data| {
        // Attempt modification
        Ok(data)
    },
    || {
        // Provide fallback value if read fails
        Ok(Default::default())
    }
).unwrap();
```

## Features

- **Thread Safety**:

  - Safe concurrent access to memory-mapped files
  - Atomic operations for data consistency
  - Lock-free reads for maximum performance

- **Type Safety**:

  - Compile-time type checking
  - Zero-copy deserialization where possible
  - Automatic validation of data integrity

- **Performance**:

  - Direct memory mapping for fast access
  - Minimized system calls
  - Efficient memory usage
  - Zero-copy operations where possible

- **Error Handling**:
  - Comprehensive error types
  - Fallback mechanisms for recovery
  - Detailed error messages
  - Safe error propagation

## Contributing Guidelines

1. **Issue First**: Create or find an issue before starting work
2. **Testing**:
   - Run benchmarks: `cargo bench`
   - Add tests for new features
   - Ensure existing tests pass
3. **Code Style**: Follow Rust standard formatting guidelines
4. **Documentation**: Update docs for API changes
5. **Performance**: No significant benchmark regressions

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## Benchmarks

Current benchmark results on standard hardware (Intel i7, 16GB RAM):

- Read operations: ~100ns
- Write operations: ~500ns
- Concurrent access: Linear scaling up to 8 threads

Run benchmarks yourself:

```bash
cargo bench
```

## Acknowledgments

- [mmap-sync](https://github.com/cloudflare/mmap-sync) for the underlying memory mapping implementation
- [rkyv](https://github.com/rkyv/rkyv) for zero-copy serialization
- All contributors who have helped improve Magic Map

---

Built with 🦀 in Rust
