import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { z } from 'zod';
import { localStorageJson } from './localStorage';

const TAB_SCHEMA = z.enum(['altitude', 'speed', 'vario']);

class MemoryLocalStorage {
  private store = new Map<string, string>();
  getItem = (key: string): string | null => this.store.get(key) ?? null;
  setItem = (key: string, value: string): void => {
    this.store.set(key, value);
  };
  removeItem = (key: string): void => {
    this.store.delete(key);
  };
  has = (key: string): boolean => this.store.has(key);
  get = (key: string): string | undefined => this.store.get(key);
}

let memoryStorage: MemoryLocalStorage;

beforeEach(() => {
  memoryStorage = new MemoryLocalStorage();
  // Vitest's node environment has no `localStorage`; install a minimal
  // in-memory shim so the helper can run as if in a browser. Each test
  // gets a fresh instance via `beforeEach`.
  Object.defineProperty(globalThis, 'localStorage', {
    value: memoryStorage,
    configurable: true,
    writable: true,
  });
});

afterEach(() => {
  delete (globalThis as { localStorage?: unknown }).localStorage;
});

describe('localStorageJson.read', () => {
  it('returns the default when the key is missing', () => {
    const out = localStorageJson.read(
      'flight-chart-tab',
      TAB_SCHEMA,
      'altitude',
    );
    expect(out).toBe('altitude');
  });

  it('returns the stored value when it parses and validates', () => {
    memoryStorage.setItem('tengri-flight-chart-tab', JSON.stringify('speed'));
    const out = localStorageJson.read(
      'flight-chart-tab',
      TAB_SCHEMA,
      'altitude',
    );
    expect(out).toBe('speed');
  });

  it('falls back to the default and removes the entry on malformed JSON', () => {
    memoryStorage.setItem('tengri-flight-chart-tab', '{not json');
    const out = localStorageJson.read(
      'flight-chart-tab',
      TAB_SCHEMA,
      'altitude',
    );
    expect(out).toBe('altitude');
    expect(memoryStorage.has('tengri-flight-chart-tab')).toBe(false);
  });

  it('falls back to the default and removes the entry on schema mismatch', () => {
    memoryStorage.setItem(
      'tengri-flight-chart-tab',
      JSON.stringify('not-a-tab'),
    );
    const out = localStorageJson.read(
      'flight-chart-tab',
      TAB_SCHEMA,
      'altitude',
    );
    expect(out).toBe('altitude');
    expect(memoryStorage.has('tengri-flight-chart-tab')).toBe(false);
  });

  it('parses non-string values via the schema', () => {
    const numberSchema = z.number().int().min(0).max(100);
    memoryStorage.setItem('tengri-volume', JSON.stringify(42));
    const out = localStorageJson.read('volume', numberSchema, 50);
    expect(out).toBe(42);
  });
});

describe('localStorageJson.write', () => {
  it('JSON-encodes the value under the prefixed key', () => {
    localStorageJson.write('flight-chart-tab', 'speed');
    expect(memoryStorage.get('tengri-flight-chart-tab')).toBe(
      JSON.stringify('speed'),
    );
  });

  it('overwrites an existing entry', () => {
    memoryStorage.setItem('tengri-flight-chart-tab', JSON.stringify('vario'));
    localStorageJson.write('flight-chart-tab', 'altitude');
    expect(memoryStorage.get('tengri-flight-chart-tab')).toBe(
      JSON.stringify('altitude'),
    );
  });

  it('round-trips through read', () => {
    localStorageJson.write('flight-chart-tab', 'vario');
    const out = localStorageJson.read(
      'flight-chart-tab',
      TAB_SCHEMA,
      'altitude',
    );
    expect(out).toBe('vario');
  });
});
