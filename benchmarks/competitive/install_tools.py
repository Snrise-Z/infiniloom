"""Tool installation and verification for competitive benchmarks."""

import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


@dataclass
class Tool:
    """Definition of a benchmarking tool."""

    name: str
    description: str
    install_command: list[str]
    check_command: list[str]
    version_command: list[str]
    pack_command_template: str  # Template with {repo_path} placeholder


# Path to infiniloom binary (built from this project)
INFINILOOM_BIN = Path(__file__).parent.parent.parent / "target" / "release" / "infiniloom"
INFINILOOM_DEBUG_BIN = Path(__file__).parent.parent.parent / "target" / "debug" / "infiniloom"

def get_infiniloom_bin() -> str:
    """Get path to infiniloom binary, preferring release build."""
    if INFINILOOM_BIN.exists():
        return str(INFINILOOM_BIN)
    if INFINILOOM_DEBUG_BIN.exists():
        return str(INFINILOOM_DEBUG_BIN)
    return "infiniloom"  # Fall back to PATH

# Tools under test
TOOLS = {
    "infiniloom": Tool(
        name="infiniloom",
        description="High-performance Rust-based repository packer",
        install_command=["cargo", "install", "--path", "cli"],
        check_command=[get_infiniloom_bin(), "--version"],
        version_command=[get_infiniloom_bin(), "--version"],
        pack_command_template=f"{get_infiniloom_bin()} pack {{repo_path}} --format xml --full",
    ),
    "repomix": Tool(
        name="repomix",
        description="Node.js repository packer",
        install_command=["npm", "install", "-g", "repomix"],
        check_command=["repomix", "--version"],
        version_command=["repomix", "--version"],
        pack_command_template="repomix {repo_path} --stdout",
    ),
    "gitingest": Tool(
        name="gitingest",
        description="Python repository packer",
        install_command=["pip", "install", "gitingest"],
        check_command=["gitingest", "--help"],
        version_command=["pip", "show", "gitingest"],
        pack_command_template="gitingest {repo_path} -o -",
    ),
}


def check_tool_installed(tool: Tool) -> tuple[bool, Optional[str]]:
    """
    Check if a tool is installed and get its version.

    Returns:
        Tuple of (is_installed, version_string)
    """
    try:
        result = subprocess.run(
            tool.check_command,
            capture_output=True,
            timeout=10,
        )
        if result.returncode == 0:
            # Try to get version
            version_result = subprocess.run(
                tool.version_command,
                capture_output=True,
                timeout=10,
            )
            version = version_result.stdout.decode().strip()
            return True, version
        return False, None
    except (subprocess.TimeoutExpired, FileNotFoundError):
        return False, None


def install_tool(tool: Tool, force: bool = False) -> bool:
    """
    Install a tool.

    Args:
        tool: Tool to install
        force: Force reinstall even if already installed

    Returns:
        True if installation successful
    """
    installed, version = check_tool_installed(tool)
    if installed and not force:
        print(f"{tool.name} already installed: {version}")
        return True

    print(f"Installing {tool.name}...")
    try:
        subprocess.run(tool.install_command, check=True)
        installed, version = check_tool_installed(tool)
        if installed:
            print(f"Successfully installed {tool.name}: {version}")
            return True
        else:
            print(f"Installation completed but {tool.name} not found in PATH")
            return False
    except subprocess.CalledProcessError as e:
        print(f"Failed to install {tool.name}: {e}")
        return False


def check_all_tools() -> dict[str, tuple[bool, Optional[str]]]:
    """Check installation status of all tools."""
    results = {}
    for name, tool in TOOLS.items():
        results[name] = check_tool_installed(tool)
    return results


def install_all_tools(force: bool = False) -> dict[str, bool]:
    """Install all tools."""
    results = {}
    for name, tool in TOOLS.items():
        results[name] = install_tool(tool, force=force)
    return results


def get_pack_command(tool_name: str, repo_path: Path) -> list[str]:
    """
    Get the pack command for a tool.

    Args:
        tool_name: Name of the tool
        repo_path: Path to repository

    Returns:
        Command as list of strings
    """
    tool = TOOLS.get(tool_name)
    if not tool:
        raise ValueError(f"Unknown tool: {tool_name}")

    cmd_str = tool.pack_command_template.format(repo_path=str(repo_path))
    return cmd_str.split()


def check_prerequisites() -> dict[str, bool]:
    """Check that required build tools are available."""
    prerequisites = {
        "cargo": ["cargo", "--version"],
        "npm": ["npm", "--version"],
        "pip": ["pip", "--version"],
        "git": ["git", "--version"],
    }

    results = {}
    for name, cmd in prerequisites.items():
        try:
            subprocess.run(cmd, capture_output=True, check=True, timeout=10)
            results[name] = True
        except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
            results[name] = False

    return results


def print_status():
    """Print installation status of all tools."""
    print("=" * 60)
    print("Prerequisites:")
    print("-" * 60)
    prereqs = check_prerequisites()
    for name, available in prereqs.items():
        status = "OK" if available else "MISSING"
        print(f"  {name:15} {status}")

    print()
    print("Tools:")
    print("-" * 60)
    tools = check_all_tools()
    for name, (installed, version) in tools.items():
        status = f"OK ({version})" if installed else "NOT INSTALLED"
        print(f"  {name:15} {status}")
    print("=" * 60)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Manage benchmark tools")
    parser.add_argument(
        "action",
        choices=["status", "install", "check"],
        help="Action to perform",
    )
    parser.add_argument(
        "--tool",
        choices=list(TOOLS.keys()),
        help="Specific tool to operate on",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Force reinstall",
    )

    args = parser.parse_args()

    if args.action == "status":
        print_status()

    elif args.action == "check":
        prereqs = check_prerequisites()
        missing = [k for k, v in prereqs.items() if not v]
        if missing:
            print(f"Missing prerequisites: {', '.join(missing)}")
            sys.exit(1)
        print("All prerequisites available")

    elif args.action == "install":
        if args.tool:
            tool = TOOLS[args.tool]
            success = install_tool(tool, force=args.force)
            sys.exit(0 if success else 1)
        else:
            results = install_all_tools(force=args.force)
            failed = [k for k, v in results.items() if not v]
            if failed:
                print(f"Failed to install: {', '.join(failed)}")
                sys.exit(1)
            print("All tools installed successfully")
