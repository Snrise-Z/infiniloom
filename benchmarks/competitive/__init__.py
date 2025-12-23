"""
Competitive Benchmarks for Infiniloom.

This package provides tools for comparing Infiniloom against
Repomix and Gitingest across multiple dimensions:
- Performance (speed, memory usage)
- Output quality (size, token efficiency)
- LLM effectiveness (code understanding, bug finding)
- Feature completeness
"""

from .repos import TEST_REPOS, clone_repo, get_repo, list_repos
from .measure import MeasurementResult, measure_command, measure_with_warmup
from .install_tools import TOOLS, check_tool_installed, get_pack_command

__all__ = [
    "TEST_REPOS",
    "clone_repo",
    "get_repo",
    "list_repos",
    "MeasurementResult",
    "measure_command",
    "measure_with_warmup",
    "TOOLS",
    "check_tool_installed",
    "get_pack_command",
]
