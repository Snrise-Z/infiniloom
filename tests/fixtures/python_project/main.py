"""Main module for test Python project."""

from dataclasses import dataclass
from typing import List, Optional


@dataclass
class User:
    """A user in the system."""

    id: int
    name: str
    email: str

    def display_name(self) -> str:
        """Get the user's display name."""
        return self.name


class UserService:
    """Service for managing users."""

    def __init__(self):
        self._users: List[User] = []

    def add_user(self, user: User) -> None:
        """Add a user to the service."""
        self._users.append(user)

    def get_user(self, user_id: int) -> Optional[User]:
        """Get a user by ID."""
        for user in self._users:
            if user.id == user_id:
                return user
        return None

    def list_users(self) -> List[User]:
        """List all users."""
        return self._users.copy()


def calculate_sum(a: int, b: int) -> int:
    """Calculate the sum of two numbers."""
    return a + b


def process_items(items: list) -> list:
    """Process a list of items."""
    return [item for item in items if item is not None]


if __name__ == "__main__":
    service = UserService()
    user = User(id=1, name="Alice", email="alice@example.com")
    service.add_user(user)
    print(f"Hello, {user.display_name()}!")
    print(f"2 + 3 = {calculate_sum(2, 3)}")
