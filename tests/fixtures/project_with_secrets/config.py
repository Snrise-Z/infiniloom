"""Configuration with secrets for security testing."""

# These are fake secrets for testing detection
AWS_ACCESS_KEY_ID = "AKIAIOSFODNN7EXAMPLE"
AWS_SECRET_ACCESS_KEY = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"

# API keys (fake)
STRIPE_API_KEY = "sk_live_FakeTestKey0000000000000"
GITHUB_TOKEN = "ghp_1234567890abcdefghijklmnopqrstuvwxyz"

# Database credentials (fake)
DATABASE_URL = "postgresql://admin:supersecretpassword@localhost:5432/mydb"

# JWT secret (fake)
JWT_SECRET = "my-super-secret-jwt-key-that-should-not-be-exposed"


def get_config():
    """Get configuration values."""
    return {
        "aws_key": AWS_ACCESS_KEY_ID,
        "debug": True,
    }
