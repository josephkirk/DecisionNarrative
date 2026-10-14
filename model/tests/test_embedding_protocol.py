import unittest

from model.embedding_protocol import EmbeddingProvider, validate_normalize_vector


class FakeEncoder:
    def __init__(self):
        self.calls = []

    def encode(self, sentences, *, prompt_name, normalize_embeddings, convert_to_numpy):
        self.calls.append((list(sentences), prompt_name, normalize_embeddings, convert_to_numpy))
        return [[3.0, 4.0, 0.0] for _ in sentences]


class EmbeddingProtocolTests(unittest.TestCase):
    def test_vectors_are_finite_fixed_size_and_unit_normalized(self):
        self.assertEqual(validate_normalize_vector([3, 4, 0], 3), [0.6, 0.8, 0.0])
        with self.assertRaisesRegex(ValueError, "dimension mismatch"):
            validate_normalize_vector([1, 0], 3)
        with self.assertRaisesRegex(ValueError, "non-finite"):
            validate_normalize_vector([1, float("inf"), 0], 3)
        with self.assertRaisesRegex(ValueError, "zero"):
            validate_normalize_vector([0, 0, 0], 3)

    def test_provider_uses_query_document_prompts_and_caches_by_revision_and_type(self):
        encoder = FakeEncoder()
        provider = EmbeddingProvider(encoder, "fixture", "revision-a", dimensions=3)

        query = provider.embed(["same", "same"], "query")
        document = provider.embed(["same"], "document")
        provider.embed(["same"], "query")

        self.assertEqual(query, [[0.6, 0.8, 0.0], [0.6, 0.8, 0.0]])
        self.assertEqual(document, [[0.6, 0.8, 0.0]])
        self.assertEqual([call[1] for call in encoder.calls], ["query", "document"])
        self.assertEqual(len(encoder.calls[0][0]), 1)

    def test_provider_rejects_empty_inputs_and_malformed_encoder_output(self):
        provider = EmbeddingProvider(FakeEncoder(), "fixture", "revision-a", dimensions=3)
        with self.assertRaisesRegex(ValueError, "one or more"):
            provider.embed([], "query")
        with self.assertRaisesRegex(ValueError, "input_type"):
            provider.embed(["x"], "unknown")

        class BrokenEncoder:
            def encode(self, *_args, **_kwargs):
                return [[1.0, 0.0]]

        broken = EmbeddingProvider(BrokenEncoder(), "fixture", "revision-a", dimensions=3)
        with self.assertRaisesRegex(ValueError, "dimension mismatch"):
            broken.embed(["x"], "query")


if __name__ == "__main__":
    unittest.main()
