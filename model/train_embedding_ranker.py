"""Fine-tune the text encoder with Unsloth and evaluate on family-held-out cases."""

from __future__ import annotations

import argparse
import json
import math
import platform
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

try:
    from .ranking_head import evaluate_pairwise_head, train_pairwise_head
    from .storylet_rank_data import RankingExample, dataset_sha256, evaluate_ranking, read_jsonl
except ImportError:  # Running this file directly from the repository root.
    from ranking_head import evaluate_pairwise_head, train_pairwise_head
    from storylet_rank_data import RankingExample, dataset_sha256, evaluate_ranking, read_jsonl


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dataset", type=Path, help="Validated JSONL containing train, validation and held_out examples")
    parser.add_argument("--output", type=Path, default=Path("model/output/storylet-embeddinggemma-2"))
    parser.add_argument("--model", default="unsloth/embeddinggemma-2")
    parser.add_argument("--model-revision", default=None, help="Optional immutable Hugging Face revision")
    parser.add_argument("--epochs", type=int, default=2)
    parser.add_argument("--batch-size", type=int, default=8)
    args = parser.parse_args()

    examples = read_jsonl(args.dataset)
    train_rows = [row for row in examples if row.split == "train"]
    validation_rows = [row for row in examples if row.split == "validation"]
    held_out_rows = [row for row in examples if row.split == "held_out"]
    if args.epochs < 1 or args.batch_size < 2:
        parser.error("--epochs must be positive and --batch-size must be at least 2 for in-batch negatives")

    try:
        from unsloth import FastSentenceTransformer, is_bf16_supported
        import torch
        from datasets import Dataset
        from sentence_transformers import SentenceTransformerTrainer, SentenceTransformerTrainingArguments
        from sentence_transformers.sentence_transformer import losses
        from sentence_transformers.training_args import BatchSamplers
    except ImportError as error:
        print(f"Training dependencies are not installed: {error}. Run uv sync --project model --extra training.", file=sys.stderr)
        return 2

    if not torch.cuda.is_available():
        print("A CUDA GPU is required for the Unsloth training run.", file=sys.stderr)
        return 2

    dtype = torch.bfloat16 if is_bf16_supported() else torch.float32
    model_kwargs = {"model_name": args.model, "max_seq_length": 1024, "dtype": dtype,
                    "full_finetuning": False, "config_kwargs": {"vision_config": None, "audio_config": None}}
    if args.model_revision:
        model_kwargs["revision"] = args.model_revision
    model = FastSentenceTransformer.from_pretrained(**model_kwargs)
    first_module = model[0]
    loaded_config = getattr(getattr(first_module, "auto_model", None), "config", None)
    resolved_revision = args.model_revision or getattr(loaded_config, "_commit_hash", None)
    if not resolved_revision:
        raise RuntimeError("could not resolve an immutable base-model revision from the loaded model")
    model = FastSentenceTransformer.get_peft_model(
        model,
        r=16,
        target_modules=["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"],
        lora_alpha=32,
        lora_dropout=0,
        bias="none",
        use_gradient_checkpointing="unsloth",
        random_state=42,
        task_type="FEATURE_EXTRACTION",
    )

    baseline_train_pairs = feature_pairs(train_rows, model)
    baseline_head = train_pairwise_head(baseline_train_pairs)
    baseline = {
        "encoder": compare_splits(model, validation_rows, held_out_rows),
        "ranking_head": {
            "validation": evaluate_pairwise_head(feature_pairs(validation_rows, model), baseline_head),
            "held_out": evaluate_pairwise_head(feature_pairs(held_out_rows, model), baseline_head),
        },
    }
    train_dataset = Dataset.from_list([{"query": row.query, "positive": row.positive, "negative": row.negative} for row in train_rows])
    validation_dataset = Dataset.from_list([{"query": row.query, "positive": row.positive, "negative": row.negative} for row in validation_rows])
    loss = losses.MultipleNegativesRankingLoss(model)
    trainer_args = SentenceTransformerTrainingArguments(
        output_dir=str(args.output / "checkpoints"),
        num_train_epochs=args.epochs,
        per_device_train_batch_size=args.batch_size,
        per_device_eval_batch_size=args.batch_size,
        learning_rate=2e-5,
        warmup_ratio=0.1,
        logging_steps=10,
        report_to="none",
        bf16=is_bf16_supported(),
        fp16=False,
        prompts={
            "query": model.prompts["query"],
            "positive": model.prompts["document"],
            "negative": model.prompts["document"],
        },
        batch_sampler=BatchSamplers.NO_DUPLICATES,
        seed=42,
    )
    trainer = SentenceTransformerTrainer(
        model=model,
        args=trainer_args,
        train_dataset=train_dataset,
        eval_dataset=validation_dataset,
        loss=loss,
    )
    training_started = time.perf_counter()
    trainer.train()
    training_seconds = time.perf_counter() - training_started

    trained_head = train_pairwise_head(feature_pairs(train_rows, model))
    trained_head["model_version"] = f"pairwise-v1:{dataset_sha256(args.dataset)[:12]}:{resolved_revision[:12]}"
    after = {
        "encoder": compare_splits(model, validation_rows, held_out_rows),
        "ranking_head": {
            "validation": evaluate_pairwise_head(feature_pairs(validation_rows, model), trained_head),
            "held_out": evaluate_pairwise_head(feature_pairs(held_out_rows, model), trained_head),
        },
    }
    args.output.mkdir(parents=True, exist_ok=True)
    model.save_pretrained(str(args.output / "adapter"))
    (args.output / "ranking-head.json").write_text(
        json.dumps(trained_head, indent=2, allow_nan=False) + "\n", encoding="utf-8"
    )
    metadata = {
        "schema_version": 1,
        "base_model": args.model,
        "base_model_revision": resolved_revision,
        "embedding_dimensions": 768,
        "task_prompts": {"query": model.prompts["query"], "document": model.prompts["document"]},
        "adapter_type": "unsloth_lora",
        "precision": "bf16" if is_bf16_supported() else "fp32",
        "dataset_sha256": dataset_sha256(args.dataset),
        "dataset_examples": {"train": len(train_rows), "validation": len(validation_rows), "held_out": len(held_out_rows)},
        "split_families": {split: sorted({row.scenario_family for row in examples if row.split == split}) for split in ("train", "validation", "held_out")},
        "label_sources": sorted({row.label_source for row in train_rows}),
        "baseline": baseline,
        "after_training": after,
        "training_seconds": training_seconds,
        "created_at": datetime.now(timezone.utc).isoformat(),
        "python": platform.python_version(),
        "torch": torch.__version__,
        "gpu": torch.cuda.get_device_name(0),
    }
    (args.output / "metadata.json").write_text(json.dumps(metadata, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    print(json.dumps(metadata, indent=2, allow_nan=False))
    return 0


def compare_splits(model, validation: list[RankingExample], held_out: list[RankingExample]) -> dict[str, dict[str, float | int]]:
    return {
        "validation": evaluate_ranking(validation, cosine(model)),
        "held_out": evaluate_ranking(held_out, cosine(model)),
    }


def feature_pairs(rows: list[RankingExample], model) -> list[dict[str, dict[str, float]]]:
    """Replace saved semantic values with similarities from the exact loaded encoder."""
    if not rows:
        raise ValueError("ranking head requires at least one example per split")
    queries = model.encode(
        [row.query for row in rows], prompt_name="query", normalize_embeddings=True, convert_to_numpy=True
    )
    passages = model.encode(
        [text for row in rows for text in (row.positive, row.negative)],
        prompt_name="document", normalize_embeddings=True, convert_to_numpy=True,
    )
    if len(queries) != len(rows) or len(passages) != len(rows) * 2:
        raise ValueError("encoder returned an unexpected number of vectors")
    dimensions = len(queries[0]) if len(queries) else 0
    if dimensions == 0 or any(len(vector) != dimensions for vector in [*queries, *passages]):
        raise ValueError("encoder returned inconsistent or empty embedding dimensions")
    result = []
    for index, row in enumerate(rows):
        positive_similarity = sum(float(a) * float(b) for a, b in zip(queries[index], passages[index * 2]))
        negative_similarity = sum(float(a) * float(b) for a, b in zip(queries[index], passages[index * 2 + 1]))
        if not math.isfinite(positive_similarity) or not math.isfinite(negative_similarity):
            raise ValueError(f"encoder returned non-finite similarity for '{row.example_id}'")
        positive_features = dict(row.positive_features)
        negative_features = dict(row.negative_features)
        positive_features["semantic_relevance"] = (max(-1.0, min(1.0, positive_similarity)) + 1.0) / 2.0
        negative_features["semantic_relevance"] = (max(-1.0, min(1.0, negative_similarity)) + 1.0) / 2.0
        result.append({"positive_features": positive_features, "negative_features": negative_features})
    return result


def cosine(model):
    def score(query: str, passage: str) -> float:
        query_embedding = model.encode(query, prompt_name="query", normalize_embeddings=True, convert_to_numpy=True)
        passage_embedding = model.encode(passage, prompt_name="document", normalize_embeddings=True, convert_to_numpy=True)
        value = float((query_embedding * passage_embedding).sum())
        if not math.isfinite(value):
            raise ValueError("encoder returned a non-finite similarity")
        return value
    return score


if __name__ == "__main__":
    raise SystemExit(main())
