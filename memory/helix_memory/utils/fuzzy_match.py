"""
helix_memory.utils.fuzzy_match

Fuzzy string matching helpers for entity deduplication and name resolution.
Wraps difflib (stdlib) with optional rapidfuzz acceleration. Populated in
Epic 3a (E3a.3).
"""

import re


def normalize_name(name: str) -> str:
    """Lowercase, strip leading/trailing whitespace, collapse internal spaces."""
    return re.sub(r"\s+", " ", name.strip().lower())


def normalize_alias(alias: str) -> str:
    """Same normalization as normalize_name."""
    return normalize_name(alias)


def names_match(a: str, b: str) -> bool:
    """Return True if two names are equal after normalization."""
    return normalize_name(a) == normalize_name(b)
