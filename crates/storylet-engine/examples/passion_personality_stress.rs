//! Personality and passion-style stress benchmark using typed tag state.
//!
//! Personality predicates are represented by authored tags. The benchmark keeps
//! the dense actor/trait workload without the removed metric/quality registry.

use std::time::Instant;
use storylet_engine::chrono_state::{GameplayTagRegistry, TagBitset};

const ACTORS: usize = 6;
const TRAITS: usize = 40;
const TRIALS: u64 = 5_000;

fn main() {
    let names: Vec<String> = (0..ACTORS)
        .flat_map(|actor| (0..TRAITS).map(move |trait_id| format!("character.{actor}.trait.{trait_id}")))
        .collect();
    let registry = GameplayTagRegistry::compile(names).unwrap();
    let mut checksum = 0u64;
    let started = Instant::now();
    for trial in 0..TRIALS {
        let mut state = TagBitset::new();
        for actor in 0..ACTORS {
            for trait_id in 0..TRAITS {
                let bit = (actor * TRAITS + trait_id) as u16;
                if (trial as usize + actor * 11 + trait_id * 3) % 7 < 3 {
                    state.set_bit(bit).unwrap();
                }
            }
        }
        // Simulate approval/hostility predicates as boolean tag intersections.
        let approved = state.bit(0) && state.bit((TRAITS + 1) as u16);
        let hostile = state.bit((2 * TRAITS + 2) as u16) && !approved;
        checksum = checksum.wrapping_add(state.words().iter().copied().sum::<u64>());
        checksum = checksum.wrapping_add(u64::from(approved) + 2 * u64::from(hostile));
    }
    println!("passion_personality trials={TRIALS} actors={ACTORS} traits={TRAITS} checksum={checksum} elapsed={:?}", started.elapsed());
    let _ = registry;
}
