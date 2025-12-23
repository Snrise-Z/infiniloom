#!/usr/bin/env python3
"""
Basic usage examples for Infiniloom Python bindings.
"""

import infiniloom
from infiniloom import Infiniloom


def functional_api_examples():
    """Examples using the functional API."""
    print("=== Functional API Examples ===\n")

    # Example 1: Pack a repository
    print("1. Packing repository...")
    context = infiniloom.pack(
        "../../",  # Infiniloom repo itself
        format="xml",
        model="claude",
        compression="balanced",
        map_budget=2000
    )
    print(f"Generated context: {len(context)} characters\n")

    # Example 2: Scan repository
    print("2. Scanning repository...")
    stats = infiniloom.scan("../../", respect_gitignore=True)
    print(f"Name: {stats['name']}")
    print(f"Files: {stats['total_files']}")
    print(f"Lines: {stats['total_lines']}")
    print(f"Claude tokens: {stats['total_tokens']['claude']}")
    print(f"Languages: {[lang['language'] for lang in stats['languages'][:3]]}")
    print()

    # Example 3: Count tokens
    print("3. Counting tokens...")
    text = "Hello, world! This is a test."
    for model in ["claude", "gpt", "gemini"]:
        tokens = infiniloom.count_tokens(text, model=model)
        print(f"  {model}: {tokens} tokens")
    print()

    # Example 4: Security scan
    print("4. Security scanning...")
    findings = infiniloom.scan_security("../../")
    print(f"Found {len(findings)} potential security issues")
    if findings:
        print(f"First finding: {findings[0]['severity']} - {findings[0]['message']}")
    print()


def oo_api_examples():
    """Examples using the object-oriented API."""
    print("\n=== Object-Oriented API Examples ===\n")

    # Create Infiniloom instance
    loom = Infiniloom("../../")
    print(f"Created: {loom}\n")

    # Example 1: Get stats
    print("1. Getting statistics...")
    stats = loom.stats()
    print(f"Repository: {stats['name']}")
    print(f"Total files: {stats['total_files']}")
    print(f"Total tokens: {stats['tokens']['claude']}\n")

    # Example 2: Get repository map
    print("2. Generating repository map...")
    repo_map = loom.map(map_budget=2000, max_symbols=10)
    print(f"Summary:\n{repo_map['summary']}\n")
    print(f"Top {len(repo_map['key_symbols'])} symbols:")
    for i, sym in enumerate(repo_map['key_symbols'][:5], 1):
        print(f"  {i}. {sym['name']} ({sym['kind']}) in {sym['file']}")
    print()

    # Example 3: List files
    print("3. Listing files...")
    files = loom.files()
    print(f"Total files: {len(files)}")

    # Group by language
    by_lang = {}
    for file in files:
        lang = file.get('language', 'unknown')
        by_lang[lang] = by_lang.get(lang, 0) + 1

    print("Files by language:")
    for lang, count in sorted(by_lang.items(), key=lambda x: x[1], reverse=True)[:5]:
        print(f"  {lang}: {count}")
    print()

    # Example 4: Pack repository
    print("4. Packing repository...")
    context = loom.pack(format="markdown", model="gpt", compression="balanced")
    print(f"Generated {len(context)} characters of context\n")

    # Example 5: Security scan
    print("5. Security scanning...")
    findings = loom.scan_security()
    if findings:
        print(f"Found {len(findings)} potential issues:")
        by_severity = {}
        for finding in findings:
            sev = finding['severity']
            by_severity[sev] = by_severity.get(sev, 0) + 1
        for sev, count in sorted(by_severity.items()):
            print(f"  {sev}: {count}")
    else:
        print("No security issues found!")
    print()


def format_examples():
    """Examples of different output formats."""
    print("\n=== Format Examples ===\n")

    loom = Infiniloom("../../")

    formats = [
        ("xml", "claude"),
        ("markdown", "gpt"),
        ("json", "claude"),
        ("yaml", "gemini"),
    ]

    for fmt, model in formats:
        print(f"Generating {fmt.upper()} for {model}...")
        context = loom.pack(format=fmt, model=model, map_budget=1000)
        print(f"  Size: {len(context)} characters")
        print(f"  Preview: {context[:100]}...")
        print()


def compression_examples():
    """Examples of different compression levels."""
    print("\n=== Compression Examples ===\n")

    loom = Infiniloom("../../")

    levels = ["none", "minimal", "balanced", "aggressive", "extreme"]

    print("Comparing compression levels:\n")
    baseline = None

    for level in levels:
        context = loom.pack(compression=level, map_budget=2000)
        size = len(context)

        if baseline is None:
            baseline = size
            reduction = 0
        else:
            reduction = (1 - size / baseline) * 100

        print(f"{level:12} {size:8} chars  ({reduction:5.1f}% reduction)")


def main():
    """Run all examples."""
    try:
        functional_api_examples()
        oo_api_examples()
        format_examples()
        compression_examples()

        print("\n All examples completed successfully!")

    except infiniloom.InfiniloomError as e:
        print(f"\n Infiniloom error: {e}")
    except Exception as e:
        print(f"\n Unexpected error: {e}")


if __name__ == "__main__":
    main()
