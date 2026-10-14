"""Loopback JSON service for query/document embeddings."""

from __future__ import annotations

import json
import os
from pathlib import Path
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Callable

try:
    from .embedding_protocol import EmbeddingProvider
    from .ranking_head import validate_ranking_head
except ImportError:  # Running this file directly with `python model/embedding_service.py`.
    from embedding_protocol import EmbeddingProvider
    from ranking_head import validate_ranking_head


class Handler(BaseHTTPRequestHandler):
    provider: EmbeddingProvider | None = None
    provider_factory: Callable[[], EmbeddingProvider] | None = None
    provider_lock = threading.Lock()

    def do_GET(self):
        if self.path == "/v1/ranking-head":
            head_path = os.environ.get("STORYLET_RANKING_HEAD")
            if not head_path:
                self._send(404, {"error": "ranking head is not configured"})
                return
            try:
                head = validate_ranking_head(json.loads(Path(head_path).read_text(encoding="utf-8")))
                self._send(200, head)
            except (OSError, json.JSONDecodeError, ValueError, TypeError) as error:
                self._send(503, {"error": f"ranking head is unavailable: {error}"})
            return
        if self.path != "/health":
            self._send(404, {"error": "not found"})
            return
        if self.provider is None:
            self._send(200, {
                "status": "not_loaded" if self.provider_factory else "unavailable",
                "model": os.environ.get("STORYLET_EMBEDDING_MODEL", "unsloth/embeddinggemma-2"),
                "dimensions": 768,
            })
            return
        self._send(200, {
            "status": "ok",
            "model": self.provider.model_id,
            "model_revision": self.provider.model_revision,
            "dimensions": self.provider.dimensions,
        })

    def do_POST(self):
        if self.path != "/v1/embeddings":
            self._send(404, {"error": "not found"})
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
            if length < 1 or length > 2_000_000:
                raise ValueError("request body size must be between 1 byte and 2 MB")
            body = json.loads(self.rfile.read(length))
            if not isinstance(body, dict):
                raise ValueError("request body must be a JSON object")
            input_value = body.get("input")
            texts = [input_value] if isinstance(input_value, str) else input_value
            if not isinstance(texts, list) or len(texts) > 64:
                raise ValueError("input must be a string or an array of at most 64 strings")
            if any(len(text) > 32_768 for text in texts if isinstance(text, str)):
                raise ValueError("each input must be at most 32768 characters")
            provider = self._get_provider()
            if provider is None:
                self._send(503, {"error": "embedding model is not configured"})
                return
            input_type = body.get("input_type", "query")
            vectors = provider.embed(texts, input_type)
            self._send(200, {
                "model": provider.model_id,
                "model_revision": provider.model_revision,
                "dimensions": provider.dimensions,
                "input_type": input_type,
                "embeddings": vectors,
            })
        except (json.JSONDecodeError, UnicodeDecodeError, ValueError, TypeError) as error:
            self._send(400, {"error": str(error)})
        except Exception as error:
            self._send(503, {"error": f"embedding service failed: {error}"})

    def _get_provider(self):
        factory = type(self).provider_factory
        if self.provider is not None or factory is None:
            return self.provider
        with self.provider_lock:
            if self.provider is None:
                self.provider = factory()
            return self.provider

    def _send(self, status: int, value: dict):
        payload = json.dumps(value, allow_nan=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, _format, *_args):
        return


def load_provider() -> EmbeddingProvider:
    from unsloth import FastSentenceTransformer
    import torch

    model_id = os.environ.get("STORYLET_EMBEDDING_MODEL", "unsloth/embeddinggemma-2")
    adapter = os.environ.get("STORYLET_EMBEDDING_ADAPTER")
    model_path = adapter or model_id
    config_kwargs = {"vision_config": None, "audio_config": None} if not adapter else {}
    model = FastSentenceTransformer.from_pretrained(
        model_name=model_path,
        for_inference=True,
        config_kwargs=config_kwargs,
    )
    first = model[0]
    config = getattr(getattr(first, "auto_model", None), "config", None)
    revision = os.environ.get("STORYLET_EMBEDDING_REVISION") or getattr(config, "_commit_hash", None)
    if not revision:
        # SentenceTransformer wrappers may omit config._commit_hash even when
        # the exact Hub snapshot is cached locally. Resolve that snapshot so
        # caches and playtest reports remain tied to immutable model weights.
        from huggingface_hub import try_to_load_from_cache

        cached_config = try_to_load_from_cache(model_path, "config.json")
        if isinstance(cached_config, str):
            snapshot = Path(cached_config).parent
            if snapshot.parent.name == "snapshots":
                revision = snapshot.name
    if not revision:
        raise RuntimeError(
            "could not determine the embedding model revision; set "
            "STORYLET_EMBEDDING_REVISION to the pinned Hub commit"
        )
    if not torch.cuda.is_available():
        model.to("cpu")
    return EmbeddingProvider(model, model_id, revision, dimensions=768)


def main() -> None:
    Handler.provider = None
    Handler.provider_factory = load_provider
    address = os.environ.get("STORYLET_EMBEDDING_ADDR", "127.0.0.1:4318")
    host, raw_port = address.rsplit(":", 1)
    if host not in ("127.0.0.1", "localhost", "::1"):
        raise SystemExit("the embedding service may only bind to loopback")
    server = ThreadingHTTPServer((host, int(raw_port)), Handler)
    print(f"Embedding service listening on {host}:{raw_port}; model loads on first request")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
