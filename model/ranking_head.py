"""Small, dependency-free pairwise linear ranking head for the Rust runtime."""

from __future__ import annotations

import math
from typing import Iterable, Mapping


FEATURE_NAMES = (
    "semantic_relevance",
    "urgency",
    "pacing_arc_alignment",
    "novelty",
    "waiting_time",
    "goal_progress",
    "role_binding_quality",
    "predicted_consequence_alignment",
    "no_action",
)


def validate_features(values: Mapping[str, object]) -> dict[str, float]:
    if not isinstance(values, Mapping):
        raise ValueError("candidate features must be an object")
    missing = set(FEATURE_NAMES) - set(values)
    unknown = set(values) - set(FEATURE_NAMES)
    if missing:
        raise ValueError(f"missing candidate features: {', '.join(sorted(missing))}")
    if unknown:
        raise ValueError(f"unsupported candidate features: {', '.join(sorted(unknown))}")
    normalized: dict[str, float] = {}
    for name in FEATURE_NAMES:
        value = values[name]
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            raise ValueError(f"feature '{name}' must be a number")
        number = float(value)
        if not math.isfinite(number):
            raise ValueError(f"feature '{name}' must be finite")
        if not 0.0 <= number <= 1.0:
            raise ValueError(f"feature '{name}' must be between 0 and 1")
        normalized[name] = number
    return normalized


def score_candidate(features: Mapping[str, object], head: Mapping[str, object]) -> float:
    vector = validate_features(features)
    validated_head = validate_ranking_head(head)
    raw_weights = validated_head["weights"]
    bias = validated_head["bias"]
    score = bias
    for name in FEATURE_NAMES:
        score += vector[name] * _finite_number(raw_weights[name], f"weight '{name}'")
    if not math.isfinite(score):
        raise ValueError("ranking head produced a non-finite score")
    return score


def validate_ranking_head(head: Mapping[str, object]) -> dict[str, object]:
    if not isinstance(head, Mapping) or head.get("schema_version") != 1:
        raise ValueError("unsupported ranking head schema")
    if not isinstance(head.get("model_version"), str) or not head["model_version"].strip():
        raise ValueError("ranking head requires a model version")
    if head.get("feature_names") != list(FEATURE_NAMES):
        raise ValueError("ranking head feature names do not match the supported schema")
    raw_weights = head.get("weights")
    if not isinstance(raw_weights, Mapping) or set(raw_weights) != set(FEATURE_NAMES):
        raise ValueError("ranking head weights must define every supported feature exactly once")
    weights = {name: _finite_number(raw_weights[name], f"weight '{name}'") for name in FEATURE_NAMES}
    bias = _finite_number(head.get("bias"), "ranking head bias")
    temperature = _finite_number(head.get("temperature"), "ranking head temperature")
    no_action_bias = _finite_number(head.get("no_action_bias"), "ranking head no-action bias")
    if temperature <= 0:
        raise ValueError("ranking head temperature must be positive")
    return {
        "schema_version": 1,
        "model_version": head["model_version"],
        "feature_names": list(FEATURE_NAMES),
        "weights": weights,
        "bias": bias,
        "temperature": temperature,
        "no_action_bias": no_action_bias,
    }


def train_pairwise_head(
    examples: Iterable[Mapping[str, object]],
    *,
    epochs: int = 300,
    learning_rate: float = 0.1,
    l2: float = 0.001,
) -> dict[str, object]:
    """Fit a pairwise logistic head; each row says positive should outrank negative."""
    rows = list(examples)
    if not rows:
        raise ValueError("ranking head training requires at least one preference pair")
    if not isinstance(epochs, int) or epochs < 1:
        raise ValueError("epochs must be a positive integer")
    learning_rate = _finite_number(learning_rate, "learning rate")
    l2 = _finite_number(l2, "L2 penalty")
    if learning_rate <= 0 or l2 < 0:
        raise ValueError("learning rate must be positive and L2 penalty cannot be negative")

    differences: list[dict[str, float]] = []
    for index, row in enumerate(rows):
        positive = validate_features(row.get("positive_features", {}))
        negative = validate_features(row.get("negative_features", {}))
        differences.append({name: positive[name] - negative[name] for name in FEATURE_NAMES})

    weights = {name: 0.0 for name in FEATURE_NAMES}
    scale = 1.0 / len(differences)
    for _ in range(epochs):
        gradient = {name: l2 * weights[name] for name in FEATURE_NAMES}
        for difference in differences:
            margin = sum(weights[name] * difference[name] for name in FEATURE_NAMES)
            pair_error = _sigmoid(-margin)
            for name in FEATURE_NAMES:
                gradient[name] -= pair_error * difference[name] * scale
        for name in FEATURE_NAMES:
            weights[name] -= learning_rate * gradient[name]

    return {
        "schema_version": 1,
        "model_version": "storylet-pairwise-v1",
        "feature_names": list(FEATURE_NAMES),
        "weights": weights,
        "bias": 0.0,
        "temperature": 1.0,
        "no_action_bias": 0.0,
    }


def evaluate_pairwise_head(examples: Iterable[Mapping[str, object]], head: Mapping[str, object]) -> dict[str, float | int]:
    rows = list(examples)
    if not rows:
        raise ValueError("ranking head evaluation requires at least one preference pair")
    correct = 0
    losses: list[float] = []
    for row in rows:
        positive = score_candidate(row.get("positive_features", {}), head)
        negative = score_candidate(row.get("negative_features", {}), head)
        margin = positive - negative
        correct += int(margin >= 0.0)
        losses.append(_softplus(-margin))
    return {"examples": len(rows), "top1_accuracy": correct / len(rows), "pairwise_log_loss": sum(losses) / len(losses)}


def _finite_number(value: object, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{label} must be a finite number")
    number = float(value)
    if not math.isfinite(number):
        raise ValueError(f"{label} must be a finite number")
    return number


def _sigmoid(value: float) -> float:
    if value >= 0:
        return 1.0 / (1.0 + math.exp(-value))
    exp_value = math.exp(value)
    return exp_value / (1.0 + exp_value)


def _softplus(value: float) -> float:
    return max(value, 0.0) + math.log1p(math.exp(-abs(value)))
