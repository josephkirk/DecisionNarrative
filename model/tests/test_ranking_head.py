import unittest

from model.ranking_head import FEATURE_NAMES, score_candidate, train_pairwise_head, validate_features
from model.train_embedding_ranker import feature_pairs
from model.storylet_rank_data import RankingExample


def features(**values):
    return {name: values.get(name, 0.5) for name in FEATURE_NAMES}


class RankingHeadTests(unittest.TestCase):
    def test_pairwise_training_learns_urgency_and_no_action_preferences(self):
        examples = [
            {"positive_features": features(urgency=0.9, no_action=0),
             "negative_features": features(urgency=0.1, no_action=1)}
            for _ in range(8)
        ]

        head = train_pairwise_head(examples, epochs=120, learning_rate=0.2, l2=0)

        self.assertGreater(head["weights"]["urgency"], 0)
        self.assertLess(head["weights"]["no_action"], 0)
        self.assertGreater(
            score_candidate(features(urgency=0.9, no_action=0), head),
            score_candidate(features(urgency=0.1, no_action=1), head),
        )

    def test_head_training_is_deterministic(self):
        examples = [{
            "positive_features": features(semantic_relevance=0.9),
            "negative_features": features(semantic_relevance=0.2),
        }]
        first = train_pairwise_head(examples, epochs=30)
        second = train_pairwise_head(examples, epochs=30)
        self.assertEqual(first, second)

    def test_feature_vectors_reject_missing_unknown_nonfinite_and_out_of_range_values(self):
        complete = features()
        with self.assertRaisesRegex(ValueError, "missing"):
            validate_features({key: value for key, value in complete.items() if key != "urgency"})
        with self.assertRaisesRegex(ValueError, "unsupported"):
            validate_features({**complete, "unreviewed": 0.2})
        with self.assertRaisesRegex(ValueError, "finite"):
            validate_features({**complete, "urgency": float("nan")})
        with self.assertRaisesRegex(ValueError, "between 0 and 1"):
            validate_features({**complete, "urgency": 1.2})

    def test_training_features_recompute_semantic_relevance_from_the_selected_encoder(self):
        class Encoder:
            def encode(self, texts, *, prompt_name, normalize_embeddings, convert_to_numpy):
                if prompt_name == "query":
                    return [[1.0, 0.0] for _ in texts]
                return [[1.0, 0.0] if text == "preferred" else [0.0, 1.0] for text in texts]

        values = {name: 0.25 for name in FEATURE_NAMES}
        row = RankingExample(
            "example", "family", "train", "rule_synthetic", "fixture", "query",
            "preferred", "alternative", values, values,
        )

        pair = feature_pairs([row], Encoder())[0]

        self.assertEqual(pair["positive_features"]["semantic_relevance"], 1.0)
        self.assertEqual(pair["negative_features"]["semantic_relevance"], 0.5)
        self.assertEqual(pair["positive_features"]["urgency"], 0.25)


if __name__ == "__main__":
    unittest.main()
