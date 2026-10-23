//! Selection profiles, gates, fallback/terminal policy, and weighted choice.

use std::collections::BTreeMap;

use crate::chrono_catalog::StoryletIndex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionPool {
    Deck,
    Qbn,
    Salience,
    Ending,
    CatalogQuery,
    Custom,
}

impl Default for SelectionPool {
    fn default() -> Self {
        Self::Deck
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionPolicy {
    #[default]
    Priority,
    SalienceMax,
    Weighted,
    SeededOrder,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TerminalPolicy {
    #[default]
    Continue,
    EndScope,
    Jump,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FallbackPolicy {
    Storylet(String),
    Continue,
    #[default]
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateKind {
    Pacing,
    Count,
    Flag,
    Metric,
    Tag,
    Custom,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectionGate {
    pub key: String,
    pub kind: GateKind,
    pub min_completed: u32,
    pub max_completed: u32,
    pub min_minutes_millis: u64,
    pub max_minutes_millis: u64,
    pub threshold: Option<f32>,
    pub shortcut_allowed: bool,
}

impl Default for SelectionGate {
    fn default() -> Self {
        Self {
            key: String::new(),
            kind: GateKind::Pacing,
            min_completed: 0,
            max_completed: 0,
            min_minutes_millis: 0,
            max_minutes_millis: 0,
            threshold: None,
            shortcut_allowed: false,
        }
    }
}

impl SelectionProfile {
    /// Canonical opt-in ending profile. The fallback storylet key is authored by
    /// the project; no ending is invented at runtime.
    pub fn ending(fallback: impl Into<String>) -> Self {
        Self {
            key: "ending".into(),
            ending: true,
            include_ending: true,
            pool: SelectionPool::Ending,
            candidate_limit: 100,
            floor: 0.0,
            gates: Vec::new(),
            policy: SelectionPolicy::Weighted,
            terminal: TerminalPolicy::EndScope,
            terminal_target: None,
            fallback: FallbackPolicy::Storylet(fallback.into()),
        }
    }

    pub fn validate(&self) -> Result<(), SelectionError> {
        if self.candidate_limit == 0 {
            return Err(SelectionError::ProfileLimitOutOfRange);
        }
        if self.floor < 0.0 || !self.floor.is_finite() {
            return Err(SelectionError::InvalidWeight);
        }
        if self.ending && self.pool != SelectionPool::Ending {
            return Err(SelectionError::InvalidGate {
                gate: "ending.pool".into(),
            });
        }
        if self.terminal == TerminalPolicy::Jump && self.terminal_target.is_none() {
            return Err(SelectionError::InvalidGate {
                gate: "terminal.target".into(),
            });
        }
        validate_gates(self)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GateCounters {
    pub completed: u32,
    pub elapsed_millis: u64,
    pub flags: BTreeMap<String, bool>,
    pub metrics: BTreeMap<String, f32>,
    pub tags: BTreeMap<String, bool>,
}

impl Default for GateCounters {
    fn default() -> Self {
        Self {
            completed: 0,
            elapsed_millis: 0,
            flags: BTreeMap::new(),
            metrics: BTreeMap::new(),
            tags: BTreeMap::new(),
        }
    }
}

impl SelectionGate {
    pub fn passes(&self, counters: &GateCounters, shortcut: bool) -> Result<(), SelectionError> {
        if self.shortcut_allowed && shortcut {
            return Ok(());
        }
        if self.min_completed > 0 && counters.completed < self.min_completed {
            return Err(SelectionError::GateRejected {
                gate: self.key.clone(),
            });
        }
        if self.max_completed > 0 && counters.completed > self.max_completed {
            return Err(SelectionError::GateRejected {
                gate: self.key.clone(),
            });
        }
        if self.min_minutes_millis > 0 && counters.elapsed_millis < self.min_minutes_millis {
            return Err(SelectionError::GateRejected {
                gate: self.key.clone(),
            });
        }
        if self.max_minutes_millis > 0 && counters.elapsed_millis > self.max_minutes_millis {
            return Err(SelectionError::GateRejected {
                gate: self.key.clone(),
            });
        }
        match self.kind {
            GateKind::Flag => {
                if counters.flags.get(&self.key).copied().unwrap_or(false) {
                    Ok(())
                } else {
                    Err(SelectionError::GateRejected {
                        gate: self.key.clone(),
                    })
                }
            }
            GateKind::Metric => {
                let threshold = self.threshold.ok_or(SelectionError::InvalidGate {
                    gate: self.key.clone(),
                })?;
                let value = counters.metrics.get(&self.key).copied().ok_or_else(|| {
                    SelectionError::InvalidGate {
                        gate: self.key.clone(),
                    }
                })?;
                if value >= threshold {
                    Ok(())
                } else {
                    Err(SelectionError::GateRejected {
                        gate: self.key.clone(),
                    })
                }
            }
            GateKind::Tag => {
                if counters.tags.get(&self.key).copied().unwrap_or(false) {
                    Ok(())
                } else {
                    Err(SelectionError::GateRejected {
                        gate: self.key.clone(),
                    })
                }
            }
            GateKind::Pacing | GateKind::Count => Ok(()),
            GateKind::Custom => Err(SelectionError::InvalidGate {
                gate: self.key.clone(),
            }),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SelectionProfile {
    pub key: String,
    pub ending: bool,
    pub include_ending: bool,
    pub pool: SelectionPool,
    pub candidate_limit: u16,
    pub floor: f32,
    pub gates: Vec<SelectionGate>,
    pub policy: SelectionPolicy,
    pub terminal: TerminalPolicy,
    pub terminal_target: Option<String>,
    pub fallback: FallbackPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionRng {
    pub seed: u64,
    pub cursor: u64,
}

impl Default for SelectionRng {
    fn default() -> Self {
        Self { seed: 0, cursor: 0 }
    }
}

impl SelectionRng {
    pub fn new(seed: u64) -> Self {
        Self { seed, cursor: 0 }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut z = self
            .seed
            .wrapping_add(self.cursor.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        self.cursor = self.cursor.wrapping_add(1);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf03_8538_4dc2_d6fc);
        z = (z ^ (z >> 27)).wrapping_mul(0x85eb_ca6b_1b31_10d2);
        z ^ (z >> 31)
    }

    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeightedCandidate {
    pub index: StoryletIndex,
    pub weight: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionOutcome {
    Selected(StoryletIndex),
    Fallback(String),
    Continue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionError {
    NoEligibleCandidate,
    InvalidWeight,
    ProfileLimitOutOfRange,
    GateRejected { gate: String },
    InvalidGate { gate: String },
}

/// One-shot weighted pick. The caller supplies candidates already filtered by
/// exact eligibility; a zero total applies the profile fallback policy.
pub fn weighted_pick_one(
    candidates: &[WeightedCandidate],
    rng: &mut SelectionRng,
) -> Result<Option<StoryletIndex>, SelectionError> {
    weighted_pick_one_with_floor(candidates, 0.0, rng)
}

pub fn weighted_pick_one_with_floor(
    candidates: &[WeightedCandidate],
    floor: f32,
    rng: &mut SelectionRng,
) -> Result<Option<StoryletIndex>, SelectionError> {
    if !floor.is_finite() || floor < 0.0 {
        return Err(SelectionError::InvalidWeight);
    }
    let mut total = 0.0f64;
    for candidate in candidates {
        if !candidate.weight.is_finite() || candidate.weight < 0.0 {
            return Err(SelectionError::InvalidWeight);
        }
        total += candidate.weight.max(floor) as f64;
    }
    if total <= 0.0 {
        return Ok(None);
    }
    let draw = rng.next_f64() * total;
    let mut acc = 0.0f64;
    for candidate in candidates {
        acc += candidate.weight.max(floor) as f64;
        if draw < acc {
            return Ok(Some(candidate.index));
        }
    }
    Ok(candidates.last().map(|candidate| candidate.index))
}

pub fn profile_gates_pass(
    profile: &SelectionProfile,
    counters: &GateCounters,
    shortcut: bool,
) -> Result<(), SelectionError> {
    for gate in &profile.gates {
        gate.passes(counters, shortcut)?;
    }
    Ok(())
}

pub fn validate_gates(profile: &SelectionProfile) -> Result<(), SelectionError> {
    for gate in &profile.gates {
        if gate.key.trim().is_empty() {
            return Err(SelectionError::InvalidGate {
                gate: gate.key.clone(),
            });
        }
        if gate.min_completed > 0
            && gate.max_completed > 0
            && gate.min_completed > gate.max_completed
        {
            return Err(SelectionError::InvalidGate {
                gate: gate.key.clone(),
            });
        }
        if gate.min_minutes_millis > 0
            && gate.max_minutes_millis > 0
            && gate.min_minutes_millis > gate.max_minutes_millis
        {
            return Err(SelectionError::InvalidGate {
                gate: gate.key.clone(),
            });
        }
        if gate.kind == GateKind::Custom {
            return Err(SelectionError::InvalidGate {
                gate: gate.key.clone(),
            });
        }
    }
    Ok(())
}

/// Resolve one profile. Priority/Salience selection is supplied by the caller
/// as an ordered candidate list; weighted selection uses the profile policy.
pub fn select_profile(
    profile: &SelectionProfile,
    ordered_candidates: &[StoryletIndex],
    weights: &BTreeMap<StoryletIndex, f32>,
    counters: &GateCounters,
    shortcut: bool,
    rng: &mut SelectionRng,
) -> Result<SelectionOutcome, SelectionError> {
    profile.validate()?;
    profile_gates_pass(profile, counters, shortcut)?;
    let limit = profile.candidate_limit as usize;
    let candidates: Vec<StoryletIndex> = ordered_candidates.iter().copied().take(limit).collect();
    if candidates.is_empty() {
        return fallback_outcome(profile, SelectionError::NoEligibleCandidate);
    }
    match profile.policy {
        SelectionPolicy::Weighted | SelectionPolicy::SeededOrder => {
            let weighted: Vec<WeightedCandidate> = candidates
                .iter()
                .map(|index| WeightedCandidate {
                    index: *index,
                    weight: weights.get(index).copied().unwrap_or(1.0),
                })
                .collect();
            match weighted_pick_one_with_floor(&weighted, profile.floor, rng)? {
                Some(index) => Ok(SelectionOutcome::Selected(index)),
                None => fallback_outcome(profile, SelectionError::NoEligibleCandidate),
            }
        }
        SelectionPolicy::Priority | SelectionPolicy::SalienceMax => {
            Ok(SelectionOutcome::Selected(candidates[0]))
        }
    }
}

fn fallback_outcome(
    profile: &SelectionProfile,
    error: SelectionError,
) -> Result<SelectionOutcome, SelectionError> {
    match &profile.fallback {
        FallbackPolicy::Storylet(key) => Ok(SelectionOutcome::Fallback(key.clone())),
        FallbackPolicy::Continue => Ok(SelectionOutcome::Continue),
        FallbackPolicy::Error => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> SelectionProfile {
        SelectionProfile {
            key: "ending".into(),
            ending: true,
            include_ending: true,
            pool: SelectionPool::Ending,
            candidate_limit: 100,
            floor: 0.0,
            gates: vec![SelectionGate {
                key: "pace".into(),
                kind: GateKind::Pacing,
                min_completed: 150,
                min_minutes_millis: 300 * 60_000,
                ..SelectionGate::default()
            }],
            policy: SelectionPolicy::Weighted,
            terminal: TerminalPolicy::EndScope,
            terminal_target: None,
            fallback: FallbackPolicy::Storylet("ending.fallback".into()),
        }
    }

    #[test]
    fn gates_are_opt_in_and_shortcut_is_explicit() {
        let profile = profile();
        let counters = GateCounters::default();
        assert!(profile_gates_pass(&profile, &counters, false).is_err());
        assert!(profile_gates_pass(&profile, &counters, true).is_err());
        let mut shortcut = profile.clone();
        shortcut.gates[0].shortcut_allowed = true;
        assert!(profile_gates_pass(&shortcut, &counters, true).is_ok());
        let mut disabled = profile.clone();
        disabled.gates[0].min_completed = 0;
        disabled.gates[0].min_minutes_millis = 0;
        assert!(profile_gates_pass(&disabled, &counters, false).is_ok());
        let at_boundary = GateCounters {
            completed: 150,
            elapsed_millis: 300 * 60_000,
            ..GateCounters::default()
        };
        assert!(profile_gates_pass(&profile, &at_boundary, false).is_ok());
    }

    #[test]
    fn ending_profile_has_canonical_defaults_and_validation() {
        let profile = SelectionProfile::ending("ending.fallback");
        assert_eq!(profile.pool, SelectionPool::Ending);
        assert_eq!(profile.terminal, TerminalPolicy::EndScope);
        assert_eq!(
            profile.fallback,
            FallbackPolicy::Storylet("ending.fallback".into())
        );
        assert!(profile.validate().is_ok());
        let mut jump = profile;
        jump.terminal = TerminalPolicy::Jump;
        assert!(jump.validate().is_err());
    }

    #[test]
    fn weighted_pick_is_deterministic_and_respects_weights() {
        let candidates = vec![
            WeightedCandidate {
                index: 4,
                weight: 3.0,
            },
            WeightedCandidate {
                index: 9,
                weight: 1.0,
            },
        ];
        let mut first = SelectionRng::new(7);
        let mut second = SelectionRng::new(7);
        assert_eq!(
            weighted_pick_one(&candidates, &mut first),
            weighted_pick_one(&candidates, &mut second)
        );
        assert_eq!(first.cursor, 1);
        assert_eq!(weighted_pick_one(&[], &mut first), Ok(None));
        assert_eq!(
            weighted_pick_one(
                &[WeightedCandidate {
                    index: 1,
                    weight: f32::NAN
                }],
                &mut first
            ),
            Err(SelectionError::InvalidWeight)
        );
    }

    #[test]
    fn profile_limit_and_fallback_policy() {
        let mut profile = profile();
        profile.gates.clear();
        profile.candidate_limit = 2;
        let mut rng = SelectionRng::new(1);
        assert_eq!(
            select_profile(
                &profile,
                &[],
                &BTreeMap::new(),
                &GateCounters::default(),
                false,
                &mut rng
            ),
            Ok(SelectionOutcome::Fallback("ending.fallback".into()))
        );
        profile.policy = SelectionPolicy::Priority;
        assert_eq!(
            select_profile(
                &profile,
                &[1, 2],
                &BTreeMap::new(),
                &GateCounters::default(),
                false,
                &mut rng
            ),
            Ok(SelectionOutcome::Selected(1))
        );
        assert_eq!(
            profile.fallback,
            FallbackPolicy::Storylet("ending.fallback".into())
        );
        assert_eq!(TerminalPolicy::EndScope, profile.terminal);
    }
}
