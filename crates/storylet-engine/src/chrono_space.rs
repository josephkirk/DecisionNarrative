//! Scheduled connections between places, the world clock and occupancy rules.
//!
//! Content (`Connection`, `Clock`) is immutable and lives in `RunContent`; the
//! only world state is `ClockState`. A project without a clock is "timeless":
//! nothing is spent and scheduled connections are always open.

use crate::chrono_participants::{FactIx, PartIx};
use crate::chrono_state::{BijectiveDelta, WorldStateTable};
use crate::run::RunContent;
use serde::{Deserialize, Serialize};

/// What happens to occupants of a place whose connection has closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OnClose {
    /// Carried to `to`.
    Return { to: u16 },
    /// Left where they are; reported so authors can see who got stuck.
    Strand,
}

/// A directed way from one place to another.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub from: u16,
    pub to: u16,
    /// Bit `n` set = open in slot `n`. `None` = always open.
    pub open_slots: Option<u64>,
    /// The mover must know this fact.
    pub requires_fact: Option<FactIx>,
    /// Applied to occupants of `to` when the connection is closed.
    pub on_close: Option<OnClose>,
}

/// The world clock: named slots and how many actions each participant gets in one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clock {
    pub slot_labels: Vec<String>,
    pub actions_per_slot: u8,
}

/// Where the clock is, and how many actions each participant has left.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockState {
    pub slot: u32,
    pub actions_left: Vec<u8>,
}

impl ClockState {
    pub fn new(clock: &Clock, participants: usize) -> Self {
        Self { slot: 0, actions_left: vec![clock.actions_per_slot; participants] }
    }
}

/// Why a move is unavailable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum MoveBlock {
    ClosedUntil { slot_label: String },
    NeedsFact { fact: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveOption {
    pub to: u16,
    pub name: String,
    pub enabled: bool,
    pub block: Option<MoveBlock>,
}

/// An occupant affected when a connection closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OccupancyEvent {
    pub participant: PartIx,
    pub from: u16,
    pub to: Option<u16>,
    pub rule: OnClose,
}

fn is_open(connection: &Connection, content: &RunContent, world: &WorldStateTable) -> bool {
    match (connection.open_slots, content.clock.as_ref(), world.clock.as_ref()) {
        (Some(mask), Some(_), Some(state)) => state.slot < 64 && mask & (1u64 << state.slot) != 0,
        // No clock: timeless, always open.
        _ => true,
    }
}

/// The label of the first slot after the current one in which `connection` is open.
fn next_open_label(connection: &Connection, content: &RunContent, world: &WorldStateTable) -> String {
    let (Some(mask), Some(clock), Some(state)) =
        (connection.open_slots, content.clock.as_ref(), world.clock.as_ref())
    else {
        return String::new();
    };
    ((state.slot + 1)..clock.slot_labels.len() as u32)
        .find(|&slot| slot < 64 && mask & (1u64 << slot) != 0)
        .map(|slot| clock.slot_labels[slot as usize].clone())
        .unwrap_or_else(|| "never".to_string())
}

fn place_name(content: &RunContent, id: u16) -> String {
    content.entity_names.get(&id).cloned().unwrap_or_else(|| id.to_string())
}

/// Every connection out of `p`'s place, with the reason a blocked one is blocked.
pub fn moves_for(world: &WorldStateTable, content: &RunContent, p: PartIx) -> Vec<MoveOption> {
    let Some(here) = world.active_location(p) else {
        return Vec::new();
    };
    content
        .connections
        .iter()
        .filter(|c| c.from == here)
        .map(|c| {
            let block = if !is_open(c, content, world) {
                Some(MoveBlock::ClosedUntil { slot_label: next_open_label(c, content, world) })
            } else if c.requires_fact.is_some_and(|fact| world.knowledge.knows(p, fact).is_none()) {
                let fact = c.requires_fact.and_then(|f| content.registry.tag(f)).unwrap_or("unknown");
                Some(MoveBlock::NeedsFact { fact: fact.to_string() })
            } else {
                None
            };
            MoveOption { to: c.to, name: place_name(content, c.to), enabled: block.is_none(), block }
        })
        .collect()
}

/// Moves the clock to the next slot, refills every participant's actions, and
/// applies `on_close` to occupants of places whose connection is now closed.
/// The clock stops at its last slot. Returns who was affected, in participant order.
pub fn advance_slot(
    world: &mut WorldStateTable,
    content: &RunContent,
    delta: &mut BijectiveDelta,
) -> Vec<OccupancyEvent> {
    let (Some(clock), Some(state)) = (content.clock.as_ref(), world.clock.as_mut()) else {
        return Vec::new();
    };
    if state.slot as usize + 1 >= clock.slot_labels.len() {
        return Vec::new();
    }
    state.slot += 1;
    for left in &mut state.actions_left {
        *left = clock.actions_per_slot;
    }
    let mut events = Vec::new();
    for p in 0..world.participants.len() as PartIx {
        let Some(at) = world.active_location(p) else { continue };
        let closing = content.connections.iter().find(|c| {
            c.to == at && c.on_close.is_some() && !is_open(c, content, world)
        });
        let Some(rule) = closing.and_then(|c| c.on_close) else { continue };
        match rule {
            OnClose::Return { to } => {
                world.apply_move(p, Some(to), delta);
                events.push(OccupancyEvent { participant: p, from: at, to: Some(to), rule });
            }
            OnClose::Strand => events.push(OccupancyEvent { participant: p, from: at, to: None, rule }),
        }
    }
    events
}
