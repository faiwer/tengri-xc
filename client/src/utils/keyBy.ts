export function keyByField<T extends object, K extends keyof T>(
  items: T[],
  field: K,
): T[K] extends string | number | symbol ? { [key in T[K]]: T } : never {
  const object: Record<PropertyKey, T> = {};
  for (const item of items) {
    if (field in item) {
      object[item[field] as PropertyKey] = item;
    }
  }

  return object as T[K] extends string | number | symbol
    ? { [key in T[K]]: T }
    : never;
}
