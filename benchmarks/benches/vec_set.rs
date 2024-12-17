use criterion::{black_box, criterion_group, criterion_main, Criterion};
use itertools::Itertools;
use std::collections::HashSet;
use uuid::Uuid; // Replace with actual usage

// Function to process using itertools::chunks
fn process_with_itertools(set: &HashSet<Uuid>) {
    set.iter().chunks(100).into_iter().for_each(|chunk| {
        // Simulate processing
        for id in chunk {
            black_box(id);
        }
    });
}

// Function to process by converting HashSet to Vec and using Vec::chunks
fn process_with_vec(set: &HashSet<Uuid>) {
    let vec_of_ids: Vec<&Uuid> = set.iter().collect();
    vec_of_ids.chunks(100).for_each(|chunk| {
        // Simulate processing
        for id in chunk {
            black_box(id);
        }
    });
}

fn process_with_vec_owned(set: &HashSet<Uuid>) {
    let vec_of_ids: Vec<Uuid> = set.iter().cloned().collect();
    vec_of_ids.chunks(100).for_each(|chunk| {
        // Simulate processing
        for id in chunk {
            black_box(id);
        }
    });
}

fn process_with_to_vec(set: &HashSet<Uuid>) {
    let vec_of_ids: Vec<Uuid> = set.into_iter().cloned().collect();
    vec_of_ids.chunks(100).for_each(|chunk| {
        // Simulate processing
        for id in chunk {
            black_box(id);
        }
    });
}

// Benchmarking function
fn benchmark(c: &mut Criterion) {
    // Create a large HashSet with 1,000,000 Uuids
    let mut hashset = HashSet::new();
    for _ in 0..1000 {
        hashset.insert(Uuid::new_v4());
    }

    // Benchmark itertools::chunks on HashSet
    c.bench_function("itertools::chunks on HashSet", |b| {
        b.iter(|| {
            process_with_itertools(black_box(&hashset));
        })
    });

    // Benchmark converting HashSet to Vec and using Vec::chunks
    c.bench_function("convert HashSet to Vec and use Vec::chunks", |b| {
        b.iter(|| {
            process_with_vec(black_box(&hashset));
        })
    });

    // Benchmark converting HashSet to Vec and using Vec::chunks
    c.bench_function("convert HashSet to Vec and use Vec::chunks owned", |b| {
        b.iter(|| {
            process_with_vec_owned(black_box(&hashset));
        })
    });

    c.bench_function("convert HashSet to Vec and use Vec::chunks process_with_to_vec", |b| {
        b.iter(|| {
            process_with_to_vec(black_box(&hashset));
        })
    });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
