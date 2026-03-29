"""
helix_memory.utils.text_processing

Text normalisation, tokenisation, and word-count utilities used by the
search and world-state modules. Populated in Epic 3a (E3a.2).
"""

import re


def word_count(text: str) -> int:
    """Count words by splitting on whitespace."""
    return len(text.split())


def truncate_to_words(text: str, max_words: int = 500) -> str:
    """Truncate text to at most max_words words.

    Appends "..." if truncation occurred.
    """
    words = text.split()
    if len(words) <= max_words:
        return text
    return " ".join(words[:max_words]) + "..."


def extract_deadlines(text: str) -> list[str]:
    """Extract date strings from context text using common deadline patterns.

    Matches:
      - "Deadline: <date>"
      - "due <date>"
      - "by <date>"

    Returns the captured date strings as-is; parsing to actual dates is
    the caller's responsibility.
    """
    patterns = [
        r"[Dd]eadline:\s*(.+?)(?:\.|,|$)",
        r"\bdue\s+(.+?)(?:\.|,|$)",
        r"\bby\s+(.+?)(?:\.|,|$)",
    ]
    results: list[str] = []
    for pattern in patterns:
        for match in re.finditer(pattern, text, re.MULTILINE):
            date_str = match.group(1).strip()
            if date_str:
                results.append(date_str)
    return results
