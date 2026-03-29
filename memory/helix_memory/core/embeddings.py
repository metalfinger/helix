"""Embedding module — singleton loader for sentence-transformers model.

Loads all-MiniLM-L6-v2 on first use. CPU-only. 384 dimensions.
"""

from __future__ import annotations

import logging
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from sentence_transformers import SentenceTransformer

logger = logging.getLogger(__name__)

# Singleton model instance
_model: SentenceTransformer | None = None
_model_name: str = "all-MiniLM-L6-v2"
EMBEDDING_DIMENSIONS = 384


def _get_model() -> SentenceTransformer:
    """Load model on first call, return cached instance after."""
    global _model
    if _model is None:
        from sentence_transformers import SentenceTransformer
        logger.info("Loading embedding model: %s", _model_name)
        _model = SentenceTransformer(_model_name, device="cpu")
        logger.info("Embedding model loaded successfully")
    return _model


def embed(text: str) -> list[float]:
    """Embed a single text string into a vector.

    Args:
        text: Text to embed

    Returns:
        384-dimensional float vector
    """
    model = _get_model()
    vector = model.encode(text, convert_to_numpy=True)
    return vector.tolist()


def embed_batch(texts: list[str]) -> list[list[float]]:
    """Embed multiple texts in a batch (more efficient than calling embed() in a loop).

    Args:
        texts: List of texts to embed

    Returns:
        List of 384-dimensional float vectors
    """
    if not texts:
        return []
    model = _get_model()
    vectors = model.encode(texts, convert_to_numpy=True, batch_size=32)
    return [v.tolist() for v in vectors]


def get_model_name() -> str:
    """Return the current embedding model name."""
    return _model_name


def configure(model_name: str = "all-MiniLM-L6-v2") -> None:
    """Configure model name before first use. Must be called before embed().

    Raises ValueError if model already loaded with different name.
    """
    global _model, _model_name
    if _model is not None and _model_name != model_name:
        raise ValueError(
            f"Embedding model already loaded as '{_model_name}', "
            f"cannot switch to '{model_name}'. Restart to change model."
        )
    _model_name = model_name
