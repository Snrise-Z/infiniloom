#!/usr/bin/env python3
"""
Examples of integrating Infiniloom with various LLM APIs.

Note: These examples require the respective LLM client libraries:
- pip install anthropic
- pip install openai
- pip install google-generativeai
"""

import infiniloom
import os


def claude_example():
    """Example integration with Anthropic Claude."""
    try:
        import anthropic
    except ImportError:
        print(" Anthropic library not installed. Run: pip install anthropic")
        return

    print("=== Claude Integration Example ===\n")

    # Generate context
    print("Generating repository context...")
    context = infiniloom.pack(
        "../../",
        format="xml",
        model="claude",
        compression="balanced",
        map_budget=2000
    )

    print(f"Context size: {len(context)} characters")
    tokens = infiniloom.count_tokens(context, model="claude")
    print(f"Estimated tokens: {tokens}\n")

    # Check for API key
    api_key = os.environ.get("ANTHROPIC_API_KEY")
    if not api_key:
        print(" ANTHROPIC_API_KEY not set. Skipping API call.")
        print("Set it with: export ANTHROPIC_API_KEY='your-key-here'")
        return

    # Send to Claude
    print("Sending to Claude API...")
    client = anthropic.Anthropic(api_key=api_key)

    try:
        response = client.messages.create(
            model="claude-sonnet-4-20250514",
            max_tokens=1024,
            messages=[{
                "role": "user",
                "content": f"{context}\n\nQuestion: What are the main components of this codebase? Provide a brief summary."
            }]
        )

        print("\nClaude's response:")
        print("-" * 60)
        print(response.content[0].text)
        print("-" * 60)

    except Exception as e:
        print(f" API call failed: {e}")


def openai_example():
    """Example integration with OpenAI GPT."""
    try:
        import openai
    except ImportError:
        print(" OpenAI library not installed. Run: pip install openai")
        return

    print("\n=== OpenAI GPT Integration Example ===\n")

    # Generate context
    print("Generating repository context...")
    context = infiniloom.pack(
        "../../",
        format="markdown",
        model="gpt",
        compression="balanced",
        map_budget=2000
    )

    print(f"Context size: {len(context)} characters")
    tokens = infiniloom.count_tokens(context, model="gpt")
    print(f"Estimated tokens: {tokens}\n")

    # Check for API key
    api_key = os.environ.get("OPENAI_API_KEY")
    if not api_key:
        print(" OPENAI_API_KEY not set. Skipping API call.")
        print("Set it with: export OPENAI_API_KEY='your-key-here'")
        return

    # Send to GPT
    print("Sending to OpenAI API...")
    client = openai.OpenAI(api_key=api_key)

    try:
        response = client.chat.completions.create(
            model="gpt-4o",
            max_tokens=1024,
            messages=[{
                "role": "user",
                "content": f"{context}\n\nQuestion: Describe the architecture of this codebase."
            }]
        )

        print("\nGPT's response:")
        print("-" * 60)
        print(response.choices[0].message.content)
        print("-" * 60)

    except Exception as e:
        print(f" API call failed: {e}")


def gemini_example():
    """Example integration with Google Gemini."""
    try:
        import google.generativeai as genai
    except ImportError:
        print(" Google AI library not installed. Run: pip install google-generativeai")
        return

    print("\n=== Google Gemini Integration Example ===\n")

    # Generate context
    print("Generating repository context...")
    context = infiniloom.pack(
        "../../",
        format="yaml",
        model="gemini",
        compression="balanced",
        map_budget=2000
    )

    print(f"Context size: {len(context)} characters")
    tokens = infiniloom.count_tokens(context, model="gemini")
    print(f"Estimated tokens: {tokens}\n")

    # Check for API key
    api_key = os.environ.get("GOOGLE_API_KEY")
    if not api_key:
        print(" GOOGLE_API_KEY not set. Skipping API call.")
        print("Set it with: export GOOGLE_API_KEY='your-key-here'")
        return

    # Send to Gemini
    print("Sending to Gemini API...")
    genai.configure(api_key=api_key)
    model = genai.GenerativeModel("gemini-1.5-pro")

    try:
        response = model.generate_content(
            f"{context}\n\nQuestion: Summarize the purpose and structure of this codebase."
        )

        print("\nGemini's response:")
        print("-" * 60)
        print(response.text)
        print("-" * 60)

    except Exception as e:
        print(f" API call failed: {e}")


def multi_model_comparison():
    """Compare responses from multiple models."""
    print("\n=== Multi-Model Comparison ===\n")

    from infiniloom import Infiniloom

    loom = Infiniloom("../../")
    stats = loom.stats()

    print(f"Repository: {stats['name']}")
    print(f"Files: {stats['total_files']}, Lines: {stats['total_lines']}\n")

    # Generate contexts for different models
    models_config = [
        ("claude", "xml", infiniloom.count_tokens),
        ("gpt", "markdown", infiniloom.count_tokens),
        ("gemini", "yaml", infiniloom.count_tokens),
    ]

    print("Token counts by model:")
    print("-" * 40)

    for model, fmt, token_counter in models_config:
        context = loom.pack(format=fmt, model=model, compression="balanced")
        tokens = token_counter(context, model=model)

        print(f"{model:8} {fmt:10} {tokens:6} tokens  ({len(context):8} chars)")

    print("-" * 40)
    print("\nNote: Actual API calls not made in comparison mode.")


def main():
    """Run all integration examples."""
    print("Infiniloom LLM Integration Examples")
    print("=" * 60)
    print()

    # Check if we have any API keys
    has_anthropic = os.environ.get("ANTHROPIC_API_KEY")
    has_openai = os.environ.get("OPENAI_API_KEY")
    has_google = os.environ.get("GOOGLE_API_KEY")

    if not any([has_anthropic, has_openai, has_google]):
        print(" No API keys found in environment variables.")
        print("Set one or more of:")
        print("  - ANTHROPIC_API_KEY")
        print("  - OPENAI_API_KEY")
        print("  - GOOGLE_API_KEY")
        print()
        print("Showing token comparison only...\n")
        multi_model_comparison()
        return

    # Run examples for available APIs
    if has_anthropic:
        claude_example()

    if has_openai:
        openai_example()

    if has_google:
        gemini_example()

    # Always show comparison
    multi_model_comparison()

    print("\n Integration examples completed!")


if __name__ == "__main__":
    main()
