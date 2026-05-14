/**
 * Group `items` into a `Map` keyed by `getKey(item)`. Insertion order is
 * preserved: a key's slot is fixed by its first occurrence, and within a bucket
 * items keep their input order.
 */
export function groupBy<T, K>(items: T[], getKey: (item: T) => K): Map<K, T[]> {
  const out = new Map<K, T[]>();
  for (const item of items) {
    const key = getKey(item);
    let bucket = out.get(key);
    if (!bucket) {
      bucket = [];
      out.set(key, bucket);
    }
    bucket.push(item);
  }
  return out;
}
