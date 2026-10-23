//! Prototype: Entity / Trait / Quality storage and Quality Effect Command
//! (QEC) cascades.
//!
//! The engine today has only global metrics, a single-pass `ZeroSumLink`
//! propagation, and no way for one quality to react to another. This example
//! builds the smallest store that answers the design questions:
//!
//! * are cascades deterministic and reversible,
//! * can cycles be rejected statically,
//! * what does one transaction cost.
//!
//! Self-contained (std only) because the engine has no equivalent types yet.
//!
//! Run: `cargo run --release --manifest-path crates/storylet-engine/Cargo.toml --example entity_quality_qec_prototype`

use std::collections::VecDeque;
use std::time::Instant;

type EntityId = u32;
type QualityId = u16;
type TraitId = u16;
type RelationId = u16;

// ------------------------------------------------------------------- schema

/// A trait is a schema: a named group of quality definitions. Granting it to
/// an entity allocates that entity's slots for those qualities.
struct TraitDef {
    key: &'static str,
    qualities: Vec<QualityId>,
}

// -------------------------------------------------------------------- store

/// Structure-of-arrays storage: one dense `f32` column with a per-entity slot
/// map, matching the data-oriented design.
struct Store {
    entity_count: usize,
    quality_count: usize,
    /// `entity * quality_count + quality` -> slot index, or `u32::MAX`.
    slot_of: Vec<u32>,
    values: Vec<f32>,
    traits_of: Vec<Vec<TraitId>>,
    /// Sorted by `(relation, entity)` so fan-out order is authored-stable.
    adjacency: Vec<Vec<(RelationId, EntityId)>>,
}

impl Store {
    fn new(entity_count: usize, quality_count: usize) -> Store {
        Store {
            entity_count,
            quality_count,
            slot_of: vec![u32::MAX; entity_count * quality_count],
            values: Vec::new(),
            traits_of: vec![Vec::new(); entity_count],
            adjacency: vec![Vec::new(); entity_count],
        }
    }

    /// `None` for an address outside the table and for a slot the entity was
    /// never granted, so a malformed command reaches the transactional
    /// `MissingSlot` path instead of aliasing another entity's slot.
    fn slot(&self, entity: EntityId, quality: QualityId) -> Option<u32> {
        let entity = entity as usize;
        let quality = quality as usize;
        if entity >= self.entity_count || quality >= self.quality_count {
            return None;
        }
        let slot = self.slot_of[entity * self.quality_count + quality];
        (slot != u32::MAX).then_some(slot)
    }

    fn value(&self, entity: EntityId, quality: QualityId) -> f32 {
        self.slot(entity, quality)
            .map(|slot| self.values[slot as usize])
            .unwrap_or(f32::NAN)
    }

    fn grant_trait(&mut self, entity: EntityId, trait_id: TraitId, def: &TraitDef) {
        for &quality in &def.qualities {
            let index = entity as usize * self.quality_count + quality as usize;
            if self.slot_of[index] == u32::MAX {
                self.slot_of[index] = self.values.len() as u32;
                self.values.push(0.0);
            }
        }
        self.traits_of[entity as usize].push(trait_id);
    }

    fn link(&mut self, from: EntityId, relation: RelationId, to: EntityId) {
        let list = &mut self.adjacency[from as usize];
        list.push((relation, to));
        list.sort_by_key(|&(relation, entity)| (relation, entity));
    }
}

// ------------------------------------------------------------- commands

enum QecOp {
    Add(f32),
    Set(f32),
}

struct Qec {
    entity: EntityId,
    quality: QualityId,
    op: QecOp,
    depth: u8,
}

/// Condition on the transition `old -> new` of the source quality.
enum When {
    AnyChange,
    /// `old < x && new >= x`
    CrossesAbove(f32),
}

impl When {
    fn holds(&self, old: f32, new: f32) -> bool {
        match self {
            When::AnyChange => true,
            When::CrossesAbove(x) => old < *x && new >= *x,
        }
    }
}

enum Target {
    SameEntity(QualityId),
    /// Every entity reachable from the changed entity over `RelationId`.
    Related(RelationId, QualityId),
    Fixed(EntityId, QualityId),
}

enum Derive {
    /// `Add(delta * k)`
    Scale(f32),
    /// `Add(k)`
    Constant(f32),
    SetTo(f32),
}

impl Derive {
    fn command(&self, delta: f32) -> QecOp {
        match self {
            Derive::Scale(k) => QecOp::Add(delta * k),
            Derive::Constant(k) => QecOp::Add(*k),
            Derive::SetTo(v) => QecOp::Set(*v),
        }
    }
}

/// The quality a subscription ultimately writes, whichever target form it uses.
fn target_quality(target: &Target) -> QualityId {
    match target {
        Target::SameEntity(quality) => *quality,
        Target::Related(_, quality) => *quality,
        Target::Fixed(_, quality) => *quality,
    }
}

struct Subscription {
    source: QualityId,
    when: When,
    target: Target,
    derive: Derive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum QecError {
    MissingSlot { entity: EntityId, quality: QualityId },
    DepthExceeded { depth: u8 },
    CommandBudgetExceeded { applied: usize },
    Cycle { quality: QualityId },
}

// ------------------------------------------------------------------- engine

struct Engine {
    /// Subscription indices per source quality, ascending by authored index.
    subs_by_source: Vec<Vec<usize>>,
    subs: Vec<Subscription>,
    max_depth: u8,
    max_commands: usize,
}

impl Engine {
    fn compile(
        subs: Vec<Subscription>,
        quality_count: usize,
        max_depth: u8,
        max_commands: usize,
    ) -> Result<Engine, QecError> {
        let mut subs_by_source: Vec<Vec<usize>> = vec![Vec::new(); quality_count];
        // Quality dependency graph: source quality -> every target quality.
        let mut dependency: Vec<Vec<QualityId>> = vec![Vec::new(); quality_count];
        // Authored ids are validated at content compile in the real design; fail
        // loudly here so a malformed subscription cannot index another quality.
        for sub in &subs {
            assert!(
                (sub.source as usize) < quality_count,
                "subscription source quality out of range"
            );
            assert!(
                (target_quality(&sub.target) as usize) < quality_count,
                "subscription target quality out of range"
            );
        }
        for (index, sub) in subs.iter().enumerate() {
            subs_by_source[sub.source as usize].push(index);
            dependency[sub.source as usize].push(target_quality(&sub.target));
        }

        // Iterative DFS with 0 = unvisited, 1 = on stack, 2 = finished.
        let mut color = vec![0u8; quality_count];
        for start in 0..quality_count {
            if color[start] != 0 {
                continue;
            }
            color[start] = 1;
            let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
            loop {
                let Some(&(node, next_index)) = stack.last() else {
                    break;
                };
                if next_index < dependency[node].len() {
                    stack.last_mut().expect("stack is non-empty").1 += 1;
                    let next = dependency[node][next_index] as usize;
                    if color[next] == 1 {
                        return Err(QecError::Cycle {
                            quality: next as QualityId,
                        });
                    }
                    if color[next] == 0 {
                        color[next] = 1;
                        stack.push((next, 0));
                    }
                } else {
                    color[node] = 2;
                    stack.pop();
                }
            }
        }

        Ok(Engine {
            subs_by_source,
            subs,
            max_depth,
            max_commands,
        })
    }

    /// Runs the cascade. On failure the store is left exactly as it was and the
    /// error is returned; on success the undo log is returned in application
    /// order as `(slot, old_value)`.
    fn execute(&self, store: &mut Store, root: Qec) -> Result<Vec<(u32, f32)>, QecError> {
        let mut queue: VecDeque<Qec> = VecDeque::new();
        queue.push_back(root);
        let mut log: Vec<(u32, f32)> = Vec::new();
        match self.drain(store, &mut queue, &mut log) {
            Ok(()) => Ok(log),
            Err(error) => {
                undo(store, &log);
                Err(error)
            }
        }
    }

    fn drain(
        &self,
        store: &mut Store,
        queue: &mut VecDeque<Qec>,
        log: &mut Vec<(u32, f32)>,
    ) -> Result<(), QecError> {
        while let Some(qec) = queue.pop_front() {
            let Some(slot) = store.slot(qec.entity, qec.quality) else {
                return Err(QecError::MissingSlot {
                    entity: qec.entity,
                    quality: qec.quality,
                });
            };
            if qec.depth > self.max_depth {
                return Err(QecError::DepthExceeded { depth: qec.depth });
            }
            if log.len() >= self.max_commands {
                return Err(QecError::CommandBudgetExceeded { applied: log.len() });
            }

            let old = store.values[slot as usize];
            let new = match qec.op {
                QecOp::Add(amount) => old + amount,
                QecOp::Set(value) => value,
            };
            store.values[slot as usize] = new;
            log.push((slot, old));

            // Compare values, not the difference: `inf - inf` is `NaN`, which
            // would report a change even though the value did not move.
            if new == old {
                continue;
            }
            let delta = new - old;

            for &sub_index in &self.subs_by_source[qec.quality as usize] {
                let sub = &self.subs[sub_index];
                if !sub.when.holds(old, new) {
                    continue;
                }
                // A child beyond `u8` can only be enqueued when the authored
                // depth cap is already absurd; report it instead of wrapping.
                let Some(child_depth) = qec.depth.checked_add(1) else {
                    return Err(QecError::DepthExceeded { depth: qec.depth });
                };
                let op = sub.derive.command(delta);
                match &sub.target {
                    Target::SameEntity(quality) => queue.push_back(Qec {
                        entity: qec.entity,
                        quality: *quality,
                        op,
                        depth: child_depth,
                    }),
                    Target::Fixed(entity, quality) => queue.push_back(Qec {
                        entity: *entity,
                        quality: *quality,
                        op,
                        depth: child_depth,
                    }),
                    Target::Related(relation, quality) => {
                        for &(edge_relation, entity) in &store.adjacency[qec.entity as usize] {
                            if edge_relation == *relation {
                                queue.push_back(Qec {
                                    entity,
                                    quality: *quality,
                                    op: match &op {
                                        QecOp::Add(amount) => QecOp::Add(*amount),
                                        QecOp::Set(value) => QecOp::Set(*value),
                                    },
                                    depth: child_depth,
                                });
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

fn undo(store: &mut Store, log: &[(u32, f32)]) {
    for &(slot, old) in log.iter().rev() {
        store.values[slot as usize] = old;
    }
}

// ----------------------------------------------------------------- scenario

const GOLD: QualityId = 0;
const REPUTATION: QualityId = 1;
const TRUST: QualityId = 2;
const STANDING: QualityId = 3;
const MENACE: QualityId = 4;
const WEALTH_FLAG: QualityId = 5;
const QUALITY_COUNT: usize = 6;

const REL_MEMBER_OF: RelationId = 0;
const REL_HAS_MEMBER: RelationId = 1;

const ENTITY_COUNT: usize = 300;
const CHARACTERS: usize = 200;
const FACTIONS: usize = 20;
const MEMBERS_PER_FACTION: usize = 10;

fn scenario_traits() -> Vec<TraitDef> {
    vec![
        TraitDef {
            key: "Wealth",
            qualities: vec![GOLD, WEALTH_FLAG],
        },
        TraitDef {
            key: "Social",
            qualities: vec![REPUTATION, TRUST],
        },
        TraitDef {
            key: "Faction",
            qualities: vec![STANDING, MENACE],
        },
    ]
}

fn build_scenario() -> Store {
    let traits = scenario_traits();
    let mut store = Store::new(ENTITY_COUNT, QUALITY_COUNT);
    for entity in 0..CHARACTERS as EntityId {
        store.grant_trait(entity, 0, &traits[0]);
        store.grant_trait(entity, 1, &traits[1]);
    }
    for index in 0..FACTIONS as EntityId {
        store.grant_trait(CHARACTERS as EntityId + index, 2, &traits[2]);
    }
    for entity in 0..CHARACTERS as EntityId {
        let faction = CHARACTERS as EntityId + entity % FACTIONS as EntityId;
        store.link(entity, REL_MEMBER_OF, faction);
        store.link(faction, REL_HAS_MEMBER, entity);
    }
    store
}

fn scenario_subs() -> Vec<Subscription> {
    vec![
        // S0: gold moves reputation.
        Subscription {
            source: GOLD,
            when: When::AnyChange,
            target: Target::SameEntity(REPUTATION),
            derive: Derive::Scale(0.1),
        },
        // S1: reputation moves faction standing of every faction I am in.
        Subscription {
            source: REPUTATION,
            when: When::AnyChange,
            target: Target::Related(REL_MEMBER_OF, STANDING),
            derive: Derive::Scale(0.5),
        },
        // S2: crossing 100 gold raises the wealth flag.
        Subscription {
            source: GOLD,
            when: When::CrossesAbove(100.0),
            target: Target::SameEntity(WEALTH_FLAG),
            derive: Derive::SetTo(1.0),
        },
        // S3: high standing makes the faction menacing.
        Subscription {
            source: STANDING,
            when: When::CrossesAbove(50.0),
            target: Target::SameEntity(MENACE),
            derive: Derive::Constant(1.0),
        },
        // S4: a menacing faction costs its members trust.
        Subscription {
            source: MENACE,
            when: When::AnyChange,
            target: Target::Related(REL_HAS_MEMBER, TRUST),
            derive: Derive::Scale(-1.0),
        },
    ]
}

fn build_engine() -> Engine {
    Engine::compile(scenario_subs(), QUALITY_COUNT, 16, 4096).expect("scenario must compile")
}

fn root(entity: EntityId, quality: QualityId, amount: f32) -> Qec {
    Qec {
        entity,
        quality,
        op: QecOp::Add(amount),
        depth: 0,
    }
}

fn main() {
    // ----------------------------------------------------------- 1. cascade
    let engine = build_engine();
    let mut store = build_scenario();
    println!(
        "scenario: entities={ENTITY_COUNT} allocated_slots={} traits_entity0={:?} traits={:?}",
        store.values.len(),
        store.traits_of[0],
        scenario_traits()
            .iter()
            .map(|trait_def| trait_def.key)
            .collect::<Vec<_>>()
    );
    let log = engine
        .execute(&mut store, root(0, GOLD, 150.0))
        .expect("cascade must succeed");
    println!(
        "cascade: applied={} gold(0)={} reputation(0)={} wealth_flag(0)={} standing(200)={}",
        log.len(),
        store.value(0, GOLD),
        store.value(0, REPUTATION),
        store.value(0, WEALTH_FLAG),
        store.value(200, STANDING)
    );
    assert_eq!(log.len(), 4);
    assert_eq!(store.value(0, GOLD), 150.0);
    assert!((store.value(0, REPUTATION) - 15.0).abs() < 1e-4);
    assert_eq!(store.value(0, WEALTH_FLAG), 1.0);
    assert!((store.value(200, STANDING) - 7.5).abs() < 1e-4);
    // Undo log order: gold, reputation, wealth_flag, standing.
    let touched: Vec<u32> = log.iter().map(|&(slot, _)| slot).collect();
    assert_eq!(
        touched,
        vec![
            store.slot(0, GOLD).unwrap(),
            store.slot(0, REPUTATION).unwrap(),
            store.slot(0, WEALTH_FLAG).unwrap(),
            store.slot(200, STANDING).unwrap(),
        ]
    );

    // ------------------------------------------------------- 2. determinism
    let mut replay_store = build_scenario();
    let replay = build_engine()
        .execute(&mut replay_store, root(0, GOLD, 150.0))
        .expect("replay must succeed");
    assert_eq!(log, replay);
    assert_eq!(store.values, replay_store.values);
    println!(
        "determinism: undo_log_identical=true final_values_identical=true values_len={}",
        replay_store.values.len()
    );

    // ------------------------------------------------------ 3. reversibility
    // Undo must restore values that were already non-zero. A log that recorded
    // the new value (or zero) would still pass the zero-baseline check below,
    // so run a second transaction on a populated store and compare the whole
    // table against the pre-transaction snapshot.
    let mut populated = build_scenario();
    let populated_engine = build_engine();
    populated_engine
        .execute(&mut populated, root(7, GOLD, 60.0))
        .expect("preparation transaction must succeed");
    assert_eq!(populated.value(7, GOLD), 60.0);
    let snapshot = populated.values.clone();
    let second_log = populated_engine
        .execute(&mut populated, root(7, GOLD, 50.0))
        .expect("second transaction must succeed");
    // Crosses the 100-gold threshold, so every touched value is non-zero.
    assert_ne!(populated.values, snapshot);
    undo(&mut populated, &second_log);
    assert_eq!(populated.values, snapshot);
    println!(
        "reversibility_nonzero: restored_snapshot=true commands={} gold_after_undo={}",
        second_log.len(),
        populated.value(7, GOLD)
    );

    // The zero-baseline path must also come back to zero.
    undo(&mut store, &log);
    for entity in 0..CHARACTERS as EntityId {
        for quality in [GOLD, REPUTATION, TRUST, WEALTH_FLAG] {
            assert_eq!(store.value(entity, quality), 0.0);
        }
    }
    for entity in CHARACTERS as EntityId..(CHARACTERS + FACTIONS) as EntityId {
        for quality in [STANDING, MENACE] {
            assert_eq!(store.value(entity, quality), 0.0);
        }
    }
    println!("reversibility: all_values_restored=true");

    // -------------------------------------------------------- 4. missing slot
    let mut missing_store = build_scenario();
    let before = missing_store.values.clone();
    let error = build_engine()
        .execute(&mut missing_store, root(250, GOLD, 1.0))
        .expect_err("location has no Wealth trait");
    println!("missing_slot: {error:?}");
    assert_eq!(
        error,
        QecError::MissingSlot {
            entity: 250,
            quality: GOLD
        }
    );
    assert_eq!(missing_store.values, before);

    // An address outside the table must be rejected, not aliased onto another
    // entity's slot: `entity * quality_count + quality` flattens both axes.
    let out_of_range_quality = QUALITY_COUNT as QualityId;
    let error = build_engine()
        .execute(&mut missing_store, root(0, out_of_range_quality, 1.0))
        .expect_err("quality beyond the table must not alias entity 1's slot");
    assert_eq!(
        error,
        QecError::MissingSlot {
            entity: 0,
            quality: out_of_range_quality
        }
    );
    let out_of_range_entity = ENTITY_COUNT as EntityId;
    let error = build_engine()
        .execute(&mut missing_store, root(out_of_range_entity, GOLD, 1.0))
        .expect_err("entity beyond the table must be rejected");
    assert_eq!(
        error,
        QecError::MissingSlot {
            entity: out_of_range_entity,
            quality: GOLD
        }
    );
    assert_eq!(missing_store.values, before);
    println!("out_of_range: quality_and_entity_addresses_rejected=true store_unchanged=true");

    // ---------------------------------------------------------- 5. static cycle
    let mut cyclic = scenario_subs();
    cyclic.push(Subscription {
        source: STANDING,
        when: When::AnyChange,
        target: Target::Fixed(0, REPUTATION),
        derive: Derive::Scale(1.0),
    });
    let cycle = Engine::compile(cyclic, QUALITY_COUNT, 16, 4096)
        .err()
        .expect("cycle must be rejected");
    println!("static_cycle: {cycle:?}");
    assert!(matches!(cycle, QecError::Cycle { .. }));

    // ------------------------------------------------------------ 6. depth cap
    const CHAIN_QUALITIES: usize = 21;
    let mut chain_store = Store::new(1, CHAIN_QUALITIES);
    let chain_trait = TraitDef {
        key: "Chain",
        qualities: (0..CHAIN_QUALITIES as QualityId).collect(),
    };
    chain_store.grant_trait(0, 0, &chain_trait);
    let chain_subs: Vec<Subscription> = (0..(CHAIN_QUALITIES - 1) as QualityId)
        .map(|quality| Subscription {
            source: quality,
            when: When::AnyChange,
            target: Target::SameEntity(quality + 1),
            derive: Derive::Constant(1.0),
        })
        .collect();
    let chain_engine =
        Engine::compile(chain_subs, CHAIN_QUALITIES, 16, 4096).expect("chain must compile");
    let depth_error = chain_engine
        .execute(&mut chain_store, root(0, 0, 1.0))
        .expect_err("chain exceeds the depth cap");
    println!("depth_cap: {depth_error:?}");
    assert_eq!(depth_error, QecError::DepthExceeded { depth: 17 });
    assert!(chain_store.values.iter().all(|&value| value == 0.0));

    // ----------------------------------------------------- 7. throughput A
    let mut linear_store = build_scenario();
    let linear_engine = build_engine();
    let transactions = 10_000u32;
    let start = Instant::now();
    let mut linear_commands = 0usize;
    for transaction in 0..transactions {
        let log = linear_engine
            .execute(
                &mut linear_store,
                root(transaction % CHARACTERS as EntityId, GOLD, 1.0),
            )
            .expect("linear cascade must succeed");
        assert_eq!(log.len(), 3);
        linear_commands += log.len();
    }
    let linear_ms = start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "throughput_linear: transactions={transactions} commands={linear_commands} total_ms={linear_ms:.3} avg_us={:.3}",
        linear_ms * 1000.0 / f64::from(transactions)
    );
    assert_eq!(linear_commands, 30_000);

    // ----------------------------------------------------- 8. throughput B
    // Routing check first: aggregate totals cannot distinguish correct faction
    // membership routing from a permutation of destinations, because every
    // faction receives the same number of transactions below.
    let mut routing_store = build_scenario();
    let routing_log = build_engine()
        .execute(&mut routing_store, root(205, MENACE, 1.0))
        .expect("routing cascade must succeed");
    assert_eq!(routing_log.len(), 1 + MEMBERS_PER_FACTION);
    let touched: Vec<usize> = (0..CHARACTERS as EntityId)
        .filter(|&entity| routing_store.value(entity, TRUST) != 0.0)
        .map(|entity| entity as usize)
        .collect();
    let expected: Vec<usize> = (0..MEMBERS_PER_FACTION).map(|member| 5 + member * 20).collect();
    assert_eq!(touched, expected);
    for entity in 0..CHARACTERS as EntityId {
        let trust = routing_store.value(entity, TRUST);
        if expected.contains(&(entity as usize)) {
            assert_eq!(trust, -1.0);
        } else {
            assert_eq!(trust, 0.0);
        }
    }
    println!("fanout_routing: touched_members={touched:?}");

    let mut fanout_store = build_scenario();
    let fanout_engine = build_engine();
    let start = Instant::now();
    let mut fanout_commands = 0usize;
    for transaction in 0..transactions {
        let entity = CHARACTERS as EntityId + transaction % FACTIONS as EntityId;
        let log = fanout_engine
            .execute(&mut fanout_store, root(entity, MENACE, 1.0))
            .expect("fan-out cascade must succeed");
        assert_eq!(log.len(), 1 + MEMBERS_PER_FACTION);
        fanout_commands += log.len();
    }
    let fanout_ms = start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "throughput_fanout: transactions={transactions} commands={fanout_commands} total_ms={fanout_ms:.3} avg_us={:.3}",
        fanout_ms * 1000.0 / f64::from(transactions)
    );
    assert_eq!(fanout_commands, 110_000);
    // Command counts alone would survive a wrong destination or a flipped sign.
    for faction in 0..FACTIONS as EntityId {
        assert_eq!(
            fanout_store.value(CHARACTERS as EntityId + faction, MENACE),
            transactions as f32 / FACTIONS as f32
        );
    }
    for entity in 0..CHARACTERS as EntityId {
        assert_eq!(
            fanout_store.value(entity, TRUST),
            -(transactions as f32 / FACTIONS as f32)
        );
    }
    println!("throughput_fanout_effects: menace_per_faction=500 trust_per_member=-500");
}
