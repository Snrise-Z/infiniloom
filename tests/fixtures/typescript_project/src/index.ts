/**
 * Main entry point for the TypeScript test project
 */

import { User, UserService } from './user';
import { formatWithPrefix, isBlank } from './utils';

/**
 * Application configuration
 */
export interface AppConfig {
  debug: boolean;
  port: number;
  host: string;
}

/**
 * Default configuration
 */
export const defaultConfig: AppConfig = {
  debug: false,
  port: 3000,
  host: 'localhost',
};

/**
 * Initialize the application
 */
export function initApp(config: Partial<AppConfig> = {}): AppConfig {
  return { ...defaultConfig, ...config };
}

/**
 * Calculate sum of two numbers
 */
export function add(a: number, b: number): number {
  return a + b;
}

/**
 * Process items in an array
 */
export function processItems<T>(items: T[]): T[] {
  return items.filter((item) => item !== null && item !== undefined);
}

// Main execution
if (require.main === module) {
  const config = initApp({ debug: true });
  const service = new UserService();

  const user = new User(1, 'Alice', 'alice@example.com');
  service.addUser(user);

  console.log(`Hello, ${user.displayName()}!`);
  console.log(`2 + 3 = ${add(2, 3)}`);
  console.log(formatWithPrefix('Config', JSON.stringify(config)));
}
