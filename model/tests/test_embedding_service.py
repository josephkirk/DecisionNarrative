import json
import threading
import os
import tempfile
import unittest
from http.server import ThreadingHTTPServer
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from model.embedding_protocol import EmbeddingProvider
from model.embedding_service import Handler
from model.ranking_head import FEATURE_NAMES, train_pairwise_head


class FakeEncoder:
    def encode(self, sentences, *, prompt_name, normalize_embeddings, convert_to_numpy):
        return [[0.0, 3.0, 4.0] for _ in sentences]


class EmbeddingHttpTests(unittest.TestCase):
    def setUp(self):
        self.old_provider = Handler.provider
        self.old_factory = Handler.provider_factory
        self.old_head_path = os.environ.pop("STORYLET_RANKING_HEAD", None)
        Handler.provider = EmbeddingProvider(FakeEncoder(), "fixture", "rev-test", dimensions=3)
        Handler.provider_factory = None
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.base = f"http://127.0.0.1:{self.server.server_address[1]}"

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=2)
        Handler.provider = self.old_provider
        Handler.provider_factory = self.old_factory
        if self.old_head_path is None:
            os.environ.pop("STORYLET_RANKING_HEAD", None)
        else:
            os.environ["STORYLET_RANKING_HEAD"] = self.old_head_path

    def test_ranking_head_endpoint_loads_and_validates_configured_artifact(self):
        head = train_pairwise_head([{
            "positive_features": {name: 0.8 for name in FEATURE_NAMES},
            "negative_features": {name: 0.2 for name in FEATURE_NAMES},
        }], epochs=2)
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", suffix=".json", delete=False) as artifact:
            json.dump(head, artifact)
            path = artifact.name
        try:
            os.environ["STORYLET_RANKING_HEAD"] = path
            with urlopen(f"{self.base}/v1/ranking-head") as response:
                self.assertEqual(json.loads(response.read()), head)
        finally:
            os.unlink(path)

    def test_ranking_head_endpoint_is_optional(self):
        with self.assertRaises(HTTPError) as error:
            urlopen(f"{self.base}/v1/ranking-head")
        self.assertEqual(error.exception.code, 404)

    def test_health_and_embedding_api_return_model_revision_and_normalized_vectors(self):
        with urlopen(f"{self.base}/health") as response:
            health = json.loads(response.read())
        self.assertEqual(health, {"status": "ok", "model": "fixture", "model_revision": "rev-test", "dimensions": 3})

        request = Request(
            f"{self.base}/v1/embeddings",
            data=json.dumps({"input": ["query", "candidate"], "input_type": "document"}).encode(),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urlopen(request) as response:
            payload = json.loads(response.read())
        self.assertEqual(payload["model_revision"], "rev-test")
        self.assertEqual(payload["input_type"], "document")
        self.assertEqual(payload["embeddings"], [[0.0, 0.6, 0.8], [0.0, 0.6, 0.8]])

    def test_malformed_requests_are_rejected(self):
        request = Request(
            f"{self.base}/v1/embeddings",
            data=json.dumps({"input": [], "input_type": "query"}).encode(),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with self.assertRaises(HTTPError) as error:
            urlopen(request)
        self.assertEqual(error.exception.code, 400)

    def test_health_does_not_block_while_loading_the_encoder(self):
        calls = []
        Handler.provider = None
        Handler.provider_factory = lambda: calls.append("load") or EmbeddingProvider(FakeEncoder(), "fixture", "rev-test", 3)
        with urlopen(f"{self.base}/health") as response:
            health = json.loads(response.read())
        self.assertEqual(health["status"], "not_loaded")
        self.assertEqual(calls, [])
        request = Request(
            f"{self.base}/v1/embeddings",
            data=json.dumps({"input": "query", "input_type": "query"}).encode(),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urlopen(request) as response:
            result = json.loads(response.read())
        self.assertEqual(calls, ["load"])
        self.assertEqual(result["model_revision"], "rev-test")


if __name__ == "__main__":
    unittest.main()
