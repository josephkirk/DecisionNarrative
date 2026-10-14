"""Small, dependency-free validation and cache boundary for encoder vectors."""

from __future__ import annotations

import math
from collections import OrderedDict
from typing import Protocol, Sequence


class TextEncoder(Protocol):
    def encode(self, sentences, *, prompt_name: str, normalize_embeddings: bool, convert_to_numpy: bool): ...


def validate_normalize_vector(values: Sequence[float], dimensions: int = 768) -> list[float]:
    vector = [float(value) for value in values]
    if len(vector) != dimensions:
        raise ValueError(f"embedding dimension mismatch: expected {dimensions}, got {len(vector)}")
    if not all(math.isfinite(value) for value in vector):
        raise ValueError("embedding contains a non-finite value")
    norm = math.sqrt(sum(value * value for value in vector))
    if not math.isfinite(norm) or norm <= 1e-12:
        raise ValueError("embedding has zero or invalid norm")
    return [value / norm for value in vector]


class EmbeddingProvider:
    def __init__(self, model: TextEncoder, model_id: str, model_revision: str, dimensions: int = 768, cache_size: int = 4096):
        self.model = model
        self.model_id = model_id
        self.model_revision = model_revision
        self.dimensions = dimensions
        self.cache_size = max(0, cache_size)
        self.cache: OrderedDict[tuple[str, str, str], list[float]] = OrderedDict()

    def embed(self, texts: Sequence[str], input_type: str) -> list[list[float]]:
        if input_type not in ("query", "document"):
            raise ValueError("input_type must be 'query' or 'document'")
        if not texts or any(not isinstance(text, str) or not text.strip() for text in texts):
            raise ValueError("input must contain one or more non-empty strings")
        keys = [(self.model_revision, input_type, text) for text in texts]
        missing = list(dict.fromkeys(key for key in keys if key not in self.cache))
        if missing:
            raw_vectors = self.model.encode(
                [key[2] for key in missing],
                prompt_name=input_type,
                normalize_embeddings=True,
                convert_to_numpy=True,
            )
            if len(raw_vectors) != len(missing):
                raise ValueError("encoder returned a different number of vectors than inputs")
            for key, vector in zip(missing, raw_vectors):
                self.cache[key] = validate_normalize_vector(vector, self.dimensions)
                self.cache.move_to_end(key)
                while len(self.cache) > self.cache_size:
                    self.cache.popitem(last=False)
        results: list[list[float]] = []
        for key in keys:
            vector = self.cache.get(key)
            if vector is None:
                # A zero-sized cache is valid; encode one item and validate it without retaining it.
                raw = self.model.encode([key[2]], prompt_name=input_type, normalize_embeddings=True, convert_to_numpy=True)
                vector = validate_normalize_vector(raw[0], self.dimensions)
            else:
                self.cache.move_to_end(key)
            results.append(vector.copy())
        return results
