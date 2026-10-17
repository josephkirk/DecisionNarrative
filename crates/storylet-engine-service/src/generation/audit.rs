//! Anti-slop prose audit rules and summarization.

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnslopAuditResult {
    pub passed: bool,
    pub violations: Vec<String>,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnslopSummary {
    pub negative_parallelism: usize,
    pub countdowns: usize,
    pub self_answered_questions: usize,
    pub buzzwords: usize,
}

const BANNED_WORDS: &[&str] = &[
    "delve",
    "tapestry",
    "landscape",
    "pivotal",
    "foster",
    "underscore",
    "testament",
    "enhance",
    "crucial",
    "intricate",
    "leverage",
    "robust",
    "streamline",
    "harness",
    "utilize",
    "multifaceted",
    "resonate",
    "catalyze",
    "quietly",
    "deeply",
    "fundamentally",
    "remarkably",
    "arguably",
    "subtly",
    "serves as",
    "stands as",
    "crucible",
    "nexus",
    "bruised silence",
    "think of it as",
    "imagine a world where",
    "here's the kicker",
    "let's break this down",
    "let's unpack",
];

pub fn audit_unslop_compliance(text: &str) -> UnslopAuditResult {
    let mut violations = Vec::new();
    let lower = text.to_lowercase();

    for &w in BANNED_WORDS {
        if lower.contains(w) {
            violations.push(format!("banned_word: '{w}'"));
        }
    }

    if let Ok(re) = Regex::new(r"(?i)nots+[^—-.]+[-—]s*it's") {
        if re.is_match(text) {
            violations.push("banned_pattern: negative_parallelism".into());
        }
    }

    if let Ok(re) = Regex::new(r"(?i)nots+[^—-.]+.s*nots+[^—-.]+.s*justs+") {
        if re.is_match(text) {
            violations.push("banned_pattern: dramatic_countdown".into());
        }
    }

    if let Ok(re) = Regex::new(r"?s+[A-Z][a-z0-9s]+.") {
        if re.is_match(text) {
            violations.push("banned_pattern: self_answered_question".into());
        }
    }

    if text.contains('—') {
        violations.push("banned_pattern: em_dash".into());
    }

    if let Ok(re) = Regex::new(r"(?i)(?:,s+)w+ing[^.!?]*[.!?](?:s|$)") {
        if re.is_match(text) {
            violations.push("banned_pattern: participial_closer".into());
        }
    }

    // Sentence burstiness calculation
    if let Ok(re) = Regex::new(r"[^.!?]+[.!?]+|[^.!?]+$") {
        let sentence_lengths: Vec<f64> = re
            .find_iter(text)
            .map(|m| m.as_str().split_whitespace().count() as f64)
            .filter(|&c| c > 0.0)
            .collect();

        if sentence_lengths.len() >= 2 {
            let n = sentence_lengths.len() as f64;
            let mean = sentence_lengths.iter().sum::<f64>() / n;
            let variance = sentence_lengths
                .iter()
                .map(|&len| (len - mean).powi(2))
                .sum::<f64>()
                / n;
            let cv = if mean == 0.0 {
                0.0
            } else {
                variance.sqrt() / mean
            };
            if cv < 0.35 {
                violations.push(format!("sentence_length_cv_below_0.35: {cv:.3}"));
            }
        }
    }

    UnslopAuditResult {
        passed: violations.is_empty(),
        violations,
        text: text.to_string(),
    }
}

pub fn summarize_unslop_violations(violations: &[String]) -> UnslopSummary {
    UnslopSummary {
        negative_parallelism: violations
            .iter()
            .filter(|v| v.contains("negative_parallelism"))
            .count(),
        countdowns: violations
            .iter()
            .filter(|v| v.contains("dramatic_countdown"))
            .count(),
        self_answered_questions: violations
            .iter()
            .filter(|v| v.contains("self_answered_question"))
            .count(),
        buzzwords: violations
            .iter()
            .filter(|v| v.starts_with("banned_word:"))
            .count(),
    }
}
