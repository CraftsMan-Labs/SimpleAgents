# Canonical copy lives at `../../workflows/rag/rag_eval_handlers.py`.
# Duplicated because `run_eval_suite` resolves handler files relative to the eval suite YAML directory.


def mock_retrieve_chunks(context, payload):
    """Workflow custom_worker: pretend a retriever returned RAG chunks."""
    return [
        {
            "source_id": "refund-policy-v3",
            "text": "Refunds are available within 30 days for eligible purchases.",
        },
        {
            "source_id": "terms-section-8",
            "text": "Refund requests require the original order id.",
        },
        {
            "source_id": "unrelated-blog-post",
            "text": "A noisy chunk that should not hurt recall.",
        },
    ]


def evaluate_rag_chunks(context, payload):
    """Custom eval handler: score retrieved source_ids against expected source ids."""
    actual = payload["actual"]
    expected = payload["expected"]
    threshold = payload.get("threshold") or 1.0

    actual_ids = {
        str(chunk.get("source_id"))
        for chunk in actual
        if isinstance(chunk, dict) and chunk.get("source_id")
    }
    expected_ids = {str(source_id) for source_id in expected}
    matched = actual_ids & expected_ids
    score = len(matched) / len(expected_ids) if expected_ids else 1.0

    return {
        "score": score,
        "passed": score >= threshold,
        "reason": f"{len(matched)}/{len(expected_ids)} expected sources matched",
        "metadata": {
            "matched": sorted(matched),
            "missing": sorted(expected_ids - actual_ids),
            "extra": sorted(actual_ids - expected_ids),
            "case_id": context.get("case_id"),
        },
    }
