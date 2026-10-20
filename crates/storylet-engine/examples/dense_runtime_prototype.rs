//! Standalone validation prototype for the dense runtime design.
//!
//! This is **not** production engine code and does not use the engine tables.
//! It models the layouts proposed in `docs/design/chrono-decision-engine.md`
//! to measure:
//! - dense storylet-completion bitset vs `BTreeSet`
//! - CSR trigger bucket lookup vs full subscription scan
//! - pre-sorted ending component resolution
//! - write-set conflict detection with tag masks
//!
//! Run:
//!   cargo run --release --manifest-path crates/storylet-engine/Cargo.toml --example dense_runtime_prototype

use std::collections::BTreeSet;
use std::hint::black_box;
use std::time::Instant;

const STORYLETS: usize = 4096;
const WORDS: usize = STORYLETS / 64;

#[derive(Clone, Copy)]
struct Bits([u64; WORDS]);
impl Default for Bits {
    fn default() -> Self {
        Self([0; WORDS])
    }
}
impl Bits {
    fn set(&mut self, i: usize) {
        self.0[i / 64] |= 1u64 << (i % 64);
    }
    fn has(&self, i: usize) -> bool {
        self.0[i / 64] & (1u64 << (i % 64)) != 0
    }
    fn clear(&mut self) {
        self.0 = [0; WORDS];
    }
}

fn splitmix(s: &mut u64) -> u64 {
    *s = s.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *s;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf0385384dc2d6fc);
    z = (z ^ (z >> 27)).wrapping_mul(0x85ebca6b1b3110d2);
    z ^ (z >> 31)
}

fn main() {
    let mut rng = 7u64;
    let mut history = Vec::with_capacity(100_000);
    for _ in 0..100_000 {
        history.push((splitmix(&mut rng) as usize) % STORYLETS);
    }

    let t = Instant::now();
    let mut bits = Bits::default();
    bits.clear();
    for &id in &history {
        bits.set(black_box(id));
    }
    black_box(bits.0);
    let bits_build = t.elapsed();

    let t = Instant::now();
    let mut set = BTreeSet::new();
    for &id in &history {
        set.insert(black_box(id));
    }
    black_box(&set);
    let set_build = t.elapsed();

    let t = Instant::now();
    let mut hits = 0usize;
    for i in 0..10_000_000usize {
        if bits.has(black_box((i * 31) % STORYLETS)) {
            hits += 1;
        }
    }
    black_box(hits);
    let bits_query = t.elapsed();

    let t = Instant::now();
    let mut set_hits = 0usize;
    for i in 0..10_000_000usize {
        if set.contains(black_box(&((i * 31) % STORYLETS))) {
            set_hits += 1;
        }
    }
    black_box(set_hits);
    let set_query = t.elapsed();

    // Trigger table: 4096 subscriptions over 64 event keys, CSR buckets.
    let mut subscriptions = Vec::new();
    for i in 0..4096usize {
        subscriptions.push((i % 64, i, (i % 17) as i16));
    }
    subscriptions.sort_by_key(|&(event, storylet, priority)| (event, priority, storylet));
    let mut offsets = vec![0usize; 65];
    for &(event, _, _) in &subscriptions {
        offsets[event + 1] += 1;
    }
    for i in 0..64 {
        offsets[i + 1] += offsets[i];
    }
    let t = Instant::now();
    let mut fired = 0usize;
    let mut bucket_checksum = 0usize;
    for i in 0..100_000usize {
        let event = (i * 13) % 64;
        for index in offsets[event]..offsets[event + 1] {
            let subscription = &subscriptions[index];
            fired += 1;
            bucket_checksum ^= subscription.1;
        }
    }
    black_box(bucket_checksum);
    let bucket_query = t.elapsed();

    let t = Instant::now();
    let mut scanned = 0usize;
    let mut scan_checksum = 0usize;
    for i in 0..100_000usize {
        let event = (i * 13) % 64;
        for subscription in &subscriptions {
            if subscription.0 == event {
                scanned += 1;
                scan_checksum ^= subscription.1;
            }
        }
    }
    black_box(scan_checksum);
    let scan_query = t.elapsed();

    // Ending components: 128 rows, 3 exclusivity groups, priority-sorted resolution.
    let mut endings: Vec<(u8, u16, i16, u32)> = (0..128)
        .map(|i| {
            (
                (i % 3) as u8,
                (i % 4) as u16,
                ((i * 7) % 31) as i16,
                i as u32,
            )
        })
        .collect();
    endings.sort_by_key(|&(group, priority, order, id)| (group, priority, order, id));
    let t = Instant::now();
    let mut winner = 0u32;
    for _ in 0..100_000usize {
        let mut used = [false; 3];
        for &(group, _, _, id) in &endings {
            if !used[group as usize] {
                used[group as usize] = true;
                winner ^= id;
            }
        }
    }
    black_box(winner);
    let ending_resolve = t.elapsed();

    // Write-set conflict: 10k transactions, tag mask intersections.
    let mut writes = Vec::with_capacity(10_000);
    for i in 0..10_000usize {
        let mut b = Bits::default();
        for k in 0..8 {
            b.set((i * 17 + k * 31) % STORYLETS);
        }
        writes.push(b);
    }
    writes[1].set(0);
    let t = Instant::now();
    let mut conflicts = 0usize;
    for i in 0..writes.len() - 1 {
        for j in i + 1..writes.len().min(i + 33) {
            if (0..WORDS).any(|w| writes[i].0[w] & writes[j].0[w] != 0) {
                conflicts += 1;
            }
        }
    }
    black_box(conflicts);
    let write_conflict = t.elapsed();

    println!(
        "bitset build {:?} | BTreeSet build {:?}",
        bits_build, set_build
    );
    println!(
        "bitset query {:?} ({} hits) | BTreeSet query {:?} ({} hits)",
        bits_query, hits, set_query, set_hits
    );
    println!(
        "trigger bucket {:?} ({} fired) | full scan {:?} ({} fired)",
        bucket_query, fired, scan_query, scanned
    );
    println!("ending resolve {:?} (winner {})", ending_resolve, winner);
    println!(
        "write-set pair conflicts {} in {:?}",
        conflicts, write_conflict
    );
}
