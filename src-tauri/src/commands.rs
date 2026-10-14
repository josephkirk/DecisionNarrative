use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ArcEvaluationResult {
    pub target_slope: f64,
    pub delta_tension: f64,
    pub arc_alignment_score: f64,
}

/// Fast Aristotelian Tension Arc alignment score calculation in Rust (Storylet.md §4.5)
/// Formula: Score = 1 / e^|target_slope - delta_t|
#[tauri::command]
pub fn evaluate_tension_arc_rust(target_slope: f64, delta_tension: f64) -> ArcEvaluationResult {
    let slope_diff = (target_slope - delta_tension).abs();
    let score = (-slope_diff).exp();
    ArcEvaluationResult {
        target_slope,
        delta_tension,
        arc_alignment_score: (score * 10000.0).round() / 10000.0,
    }
}

/// Fast salience matching score across qualities
#[tauri::command]
pub fn compute_salience_rust(matches_count: u32, is_local_node: bool) -> f64 {
    let base = (matches_count as f64) * 10.0;
    if is_local_node {
        base + 15.0
    } else {
        base
    }
}
