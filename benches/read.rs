use criterion::{criterion_group, criterion_main, Criterion};
use magic_map::MagicMap;
use rand::{distributions::Alphanumeric, Rng};
use rkyv::{bytecheck, Archive, CheckBytes, Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

const FILE_NAME: &str = "/tmp/test_magicmap";

#[derive(Archive, Debug, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
struct TestStructComplex {
    a: u32,
    b: HashMap<String, u32>,
}

fn bench_magic_map(c: &mut Criterion) {
    let mmap = MagicMap::<TestStructComplex>::new(PathBuf::from(format!("{}_rw", FILE_NAME)));
    let gen_hash_map = |n| {
        let mut container = HashMap::new();
        for i in 0..n {
            let s: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(24)
                .map(char::from)
                .collect();
            container.insert(s, i);
        }
        container
    };

    let new_data = |n| {
        let dat = gen_hash_map(n);
        TestStructComplex {
            a: u32::max_value(),
            b: dat,
        }
    };
    mmap.clone()
        .create(new_data(500))
        .expect("failed to create data");

    let iter = 10000;

    c.bench_function("read", |b| {
        b.iter(|| {
            for _i in 0..iter {
                mmap.clone().read(|_d| Ok(())).expect("read failed");
            }
        })
    });
}

criterion_group!(benches, bench_magic_map);

criterion_main!(benches);
