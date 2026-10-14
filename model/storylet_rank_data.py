"""Validated, provenance-aware data contract for narrative ranking examples."""

from __future__ import annotations

import hashlib
import json
import math
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Iterable, Literal

try:
    from .ranking_head import validate_features
except ImportError:  # Running the training script directly from the model folder.
    from ranking_head import validate_features

Split = Literal["train", "validation", "held_out"]
LabelSource = Literal["human_reviewed", "rule_synthetic", "strata_synthetic"]


@dataclass(frozen=True)
class RankingExample:
    example_id: str
    scenario_family: str
    split: Split
    label_source: LabelSource
    provenance: str
    query: str
    positive: str
    negative: str
    positive_features: dict[str, float]
    negative_features: dict[str, float]

    @classmethod
    def from_dict(cls, value: dict) -> "RankingExample":
        fields = ("example_id", "scenario_family", "split", "label_source", "provenance", "query", "positive", "negative")
        missing = [name for name in fields if not isinstance(value.get(name), str) or not value[name].strip()]
        if missing:
            raise ValueError(f"missing or empty fields: {', '.join(missing)}")
        if value["split"] not in ("train", "validation", "held_out"):
            raise ValueError(f"unsupported split '{value['split']}'")
        if value["label_source"] not in ("human_reviewed", "rule_synthetic", "strata_synthetic"):
            raise ValueError(f"unsupported label_source '{value['label_source']}'")
        if value["split"] != "train" and value["label_source"] != "human_reviewed":
            raise ValueError("validation and held_out labels must be human_reviewed")
        try:
            positive_features = validate_features(value.get("positive_features", {}))
            negative_features = validate_features(value.get("negative_features", {}))
        except ValueError as error:
            raise ValueError(f"invalid candidate features: {error}") from error
        return cls(**{name: value[name] for name in fields},
                   positive_features=positive_features, negative_features=negative_features)


def read_jsonl(path: str | Path) -> list[RankingExample]:
    examples: list[RankingExample] = []
    with Path(path).open("r", encoding="utf-8") as source:
        for line_number, line in enumerate(source, 1):
            if not line.strip():
                continue
            try:
                value = json.loads(line)
                if not isinstance(value, dict):
                    raise ValueError("expected a JSON object")
                examples.append(RankingExample.from_dict(value))
            except (json.JSONDecodeError, ValueError) as error:
                raise ValueError(f"{path}:{line_number}: {error}") from error
    validate_examples(examples)
    return examples


def validate_examples(examples: Iterable[RankingExample]) -> None:
    items = list(examples)
    if not items:
        raise ValueError("dataset must contain at least one example")
    ids: set[str] = set()
    families: dict[str, set[str]] = defaultdict(set)
    for example in items:
        if example.example_id in ids:
            raise ValueError(f"duplicate example_id '{example.example_id}'")
        ids.add(example.example_id)
        families[example.scenario_family].add(example.split)
    leaked = sorted(family for family, splits in families.items() if len(splits) > 1)
    if leaked:
        raise ValueError(f"scenario families cross dataset splits: {', '.join(leaked)}")
    if not any(example.split == "train" for example in items):
        raise ValueError("dataset has no training examples")
    if not any(example.split == "validation" for example in items):
        raise ValueError("dataset has no validation examples")
    if not any(example.split == "held_out" for example in items):
        raise ValueError("dataset has no held_out examples")


def dataset_sha256(path: str | Path) -> str:
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def evaluate_ranking(
    examples: Iterable[RankingExample],
    similarity: Callable[[str, str], float],
) -> dict[str, float | int]:
    """Evaluate pairwise choices with MRR, top-1 accuracy and binary NDCG@2."""
    rows = list(examples)
    if not rows:
        raise ValueError("evaluation requires at least one example")
    reciprocal_ranks: list[float] = []
    top_one: list[float] = []
    ndcg: list[float] = []
    for row in rows:
        positive_score = float(similarity(row.query, row.positive))
        negative_score = float(similarity(row.query, row.negative))
        if not math.isfinite(positive_score) or not math.isfinite(negative_score):
            raise ValueError(f"non-finite similarity for example '{row.example_id}'")
        positive_wins = positive_score >= negative_score
        reciprocal_ranks.append(1.0 if positive_wins else 0.5)
        top_one.append(1.0 if positive_wins else 0.0)
        ndcg.append(1.0 if positive_wins else 1.0 / math.log2(3))
    return {
        "examples": len(rows),
        "mrr_at_2": sum(reciprocal_ranks) / len(rows),
        "top1_accuracy": sum(top_one) / len(rows),
        "ndcg_at_2": sum(ndcg) / len(rows),
    }
