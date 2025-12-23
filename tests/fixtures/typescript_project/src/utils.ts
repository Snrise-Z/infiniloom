/**
 * Utility functions
 */

/**
 * Format a string with a prefix
 */
export function formatWithPrefix(prefix: string, value: string): string {
  return `${prefix}: ${value}`;
}

/**
 * Check if a string is empty or whitespace
 */
export function isBlank(s: string): boolean {
  return !s || s.trim().length === 0;
}

/**
 * Merge two objects
 */
export function mergeObjects<T extends object>(a: T, b: Partial<T>): T {
  return { ...a, ...b };
}

/**
 * Delay execution
 */
export function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
