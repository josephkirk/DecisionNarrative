//! Tag-based gas-station stress benchmark.
//!
//! This intentionally exercises the current boolean world-state contract: tags
//! are the eligibility and progression state; numeric metric gates are not used.

use std::time::Instant;
use storylet_engine::chrono_state::{GameplayTagRegistry, TagBitset};

const ACTORS: usize = 60;
const TRIALS: u64 = 500;
const TURNS: u64 = 32;

fn main() {
    let names: Vec<String> = (0..128).map(|i| format!("station.flag.{i}")).collect();
    let registry = GameplayTagRegistry::compile(names).unwrap();
    let mut total = 0u64;
    let started = Instant::now();
    for trial in 0..TRIALS {
        let mut state = TagBitset::new();
        for turn in 0..TURNS {
            for actor in 0..ACTORS {
                let id = ((trial as usize + turn as usize * 3 + actor * 7) % 128) as u16;
                state.set_bit(id).unwrap();
                if (turn + actor as u64) % 5 == 0 {
                    state.clear_bit(id).unwrap();
                }
            }
            total = total.wrapping_add(state.words().iter().copied().sum::<u64>());
        }
    }
    println!("gas_station trials={TRIALS} actors={ACTORS} turns={TURNS} checksum={total} elapsed={:?}", started.elapsed());
    let _ = registry;
}
