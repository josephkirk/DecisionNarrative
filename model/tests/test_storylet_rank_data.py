import unittest

from model.storylet_rank_data import RankingExample, evaluate_ranking, validate_examples
from model.ranking_head import FEATURE_NAMES


def example(example_id: str, family: str, split: str, label_source: str = "human_reviewed") -> RankingExample:
    scores = {name: 0.5 for name in FEATURE_NAMES}
    return RankingExample(example_id, family, split, label_source, "fixture", "query", "positive", "negative", scores, scores)


class RankingDataTests(unittest.TestCase):
    def test_split_families_are_disjoint_and_all_splits_are_present(self):
        validate_examples([
            example("tr-1", "detective-a", "train", "rule_synthetic"),
            example("va-1", "detective-b", "validation"),
            example("ho-1", "fantasy-a", "held_out"),
        ])

    def test_strata_synthetic_labels_cannot_enter_validation_or_held_out(self):
        with self.assertRaisesRegex(ValueError, "must be human_reviewed"):
            RankingExample.from_dict({
                "example_id": "ho-1", "scenario_family": "fantasy-a", "split": "held_out",
                "label_source": "strata_synthetic", "provenance": "model", "query": "q",
                "positive": "p", "negative": "n",
                "positive_features": {name: 0.5 for name in FEATURE_NAMES},
                "negative_features": {name: 0.5 for name in FEATURE_NAMES},
            })

    def test_family_leakage_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "cross dataset splits"):
            validate_examples([
                example("tr-1", "same-family", "train"),
                example("va-1", "same-family", "validation"),
                example("ho-1", "other-family", "held_out"),
            ])

    def test_metrics_check_finite_scores_and_measure_positive_ranking(self):
        rows = [example("ho-1", "family", "held_out")]
        metrics = evaluate_ranking(rows, lambda _query, candidate: 0.8 if candidate == "positive" else 0.2)
        self.assertEqual(metrics["top1_accuracy"], 1.0)
        self.assertEqual(metrics["mrr_at_2"], 1.0)
        with self.assertRaisesRegex(ValueError, "non-finite"):
            evaluate_ranking(rows, lambda _query, _candidate: float("nan"))


if __name__ == "__main__":
    unittest.main()
