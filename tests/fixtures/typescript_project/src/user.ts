/**
 * User module
 */

/**
 * Represents a user in the system
 */
export class User {
  constructor(
    public readonly id: number,
    public readonly name: string,
    public readonly email: string
  ) {}

  /**
   * Get the user's display name
   */
  displayName(): string {
    return this.name;
  }

  /**
   * Convert user to JSON
   */
  toJSON(): object {
    return {
      id: this.id,
      name: this.name,
      email: this.email,
    };
  }
}

/**
 * Service for managing users
 */
export class UserService {
  private users: User[] = [];

  /**
   * Add a user to the service
   */
  addUser(user: User): void {
    this.users.push(user);
  }

  /**
   * Get a user by ID
   */
  getUser(userId: number): User | undefined {
    return this.users.find((u) => u.id === userId);
  }

  /**
   * List all users
   */
  listUsers(): User[] {
    return [...this.users];
  }

  /**
   * Remove a user by ID
   */
  removeUser(userId: number): boolean {
    const index = this.users.findIndex((u) => u.id === userId);
    if (index !== -1) {
      this.users.splice(index, 1);
      return true;
    }
    return false;
  }
}
