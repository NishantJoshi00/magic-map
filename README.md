## Magic Map

A wrapper abstraction over [mmap-sync](https://github.com/cloudflare/mmap-sync) that provides a thread safe object to be used in a multi-threaded/asynchronous environment.

### Installation

To use this library, add the following to your `Cargo.toml`:

```toml
[dependencies]
magic-map = "0.1.0"

rkyv = { version = "0.7.40", features = ["validation", "strict"] }
bytecheck = { version = "~0.6.8", default-features = false }
```

- `rkyv`: This is the serialization library used to serialize and deserialize the data.
- `bytecheck`: This is the library used to check the data integrity.

### Usage

```rust
use magic_map::MagicMap;
use rkyv::{Archive, Serialize, Deserialize};
use bytecheck::CheckBytes;

#[derive(Archive, Debug, Serialize, Deserialize, Clone)]
#[archive_attr(derive(CheckBytes))]
struct Test {
    a: u32,
    b: u32,
}

fn main() {
    let mut magic_map = MagicMap::new("test.bin".into());
    let data = Test { a: 1, b: 2 };
    magic_map.clone().create(data.clone()).unwrap();

    let r_data = magic_map.get_owned().unwrap();

    assert_eq!(data, r_data);
}
```

In the example mentioned above, we have created a `MagicMap` object and created a `Test` object and stored it in the `MagicMap`. We then retrieved the object from the `MagicMap` and checked if the data is the same as the original data.

When calling [`Clone::clone`] on the `MagicMap` object, this performs a shallow copy of the synchronizer maintaining the same memory mapping. This allows the `MagicMap` object to be shared across threads/contexts.


