"""Utility functions for the project."""

from typing import Any, Dict


def format_with_prefix(prefix: str, value: str) -> str:
    """Format a string with a prefix."""
    return f"{prefix}: {value}"


def is_blank(s: str) -> bool:
    """Check if a string is empty or whitespace."""
    return not s or s.isspace()


def merge_dicts(a: Dict[str, Any], b: Dict[str, Any]) -> Dict[str, Any]:
    """Merge two dictionaries."""
    result = a.copy()
    result.update(b)
    return result
