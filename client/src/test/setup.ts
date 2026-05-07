/**
 * Vitest setup. Polyfills the browser timing APIs that some modules under
 * test use directly (`runWithRaf` requires `requestAnimationFrame`); the
 * Node test environment doesn't ship them. The polyfill schedules via
 * `setTimeout(0)` — fine for tests because the work is small and we don't
 * care about animation pacing here.
 */

if (typeof globalThis.requestAnimationFrame !== 'function') {
  globalThis.requestAnimationFrame = (cb: FrameRequestCallback): number =>
    setTimeout(() => cb(performance.now()), 0) as unknown as number;
  globalThis.cancelAnimationFrame = (id: number): void => {
    clearTimeout(id as unknown as ReturnType<typeof setTimeout>);
  };
}
