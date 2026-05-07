/**
 * Run a long-running synchronous job in time-sliced chunks via
 * `requestAnimationFrame`, yielding to paint between slices.
 *
 * The job is a `step()` callable that advances the work by one unit; the
 * runner repeatedly invokes it until `isDone()` returns true, sampling the
 * wall clock every `checkEvery` steps and bailing out of the current frame
 * once the per-frame budget is spent. The next frame picks up where this
 * one left off.
 *
 * Self-tunes to the host: a slow CPU completes fewer steps per frame, a fast
 * one completes more — both stay inside `frameBudgetMs`. Caller does not
 * pick a chunk size; it picks a time budget.
 */
export interface RunWithRafOptions {
  /** Advance the job by one indivisible unit. Throwing rejects the promise. */
  step: () => void;
  /** True when no more work remains. Called between every step. */
  isDone: () => boolean;
  /**
   * Wall-clock budget per animation frame, in milliseconds. Default 5 ms —
   * fits inside a 120 Hz frame (~8.3 ms) and leaves >half of a 60 Hz frame
   * (~16.7 ms) for paint.
   */
  frameBudgetMs?: number;
  /**
   * How often to sample `performance.now()`, in steps. Sampling every step
   * has measurable overhead in tight loops (clock read ≈ tens of ns); this
   * caps overshoot at `checkEvery × per-step cost` while amortizing the
   * read. Must be a power of two so the modulo folds into a bitmask.
   * Default 1024.
   */
  checkEvery?: number;
  /** Cancel an in-flight run; rejects with `AbortError`. */
  signal?: AbortSignal;
  /**
   * Called once per frame after the slice. Useful for progress bars; called
   * with whatever values the caller wants to surface (e.g. processed count).
   */
  onFrame?: () => void;
}

const DEFAULT_BUDGET_MS = 5;
const DEFAULT_CHECK_EVERY = 1024;

export function runWithRaf({
  step,
  isDone,
  frameBudgetMs = DEFAULT_BUDGET_MS,
  checkEvery = DEFAULT_CHECK_EVERY,
  signal,
  onFrame,
}: RunWithRafOptions): Promise<void> {
  if (!isPow2(checkEvery)) {
    return Promise.reject(
      new RangeError(
        `runWithRaf: checkEvery must be a power of two, got ${checkEvery}`,
      ),
    );
  }

  if (signal?.aborted) {
    return Promise.reject(abortError());
  }

  if (isDone()) {
    return Promise.resolve();
  }

  return new Promise<void>((resolve, reject) => {
    let rafId = 0;

    const onAbort = () => {
      cancelAnimationFrame(rafId);
      reject(abortError());
    };

    if (signal) {
      signal.addEventListener('abort', onAbort, { once: true });
    }

    const cleanup = () => {
      if (signal) {
        signal.removeEventListener('abort', onAbort);
      }
    };

    const mask = checkEvery - 1;

    const tick = (frameStart: DOMHighResTimeStamp): void => {
      const deadline = frameStart + frameBudgetMs;
      let n = 0;
      try {
        while (!isDone()) {
          step();
          n++;
          if ((n & mask) === 0 && performance.now() >= deadline) {
            break;
          }
        }
      } catch (err) {
        cleanup();
        reject(err);
        return;
      }

      onFrame?.();

      if (isDone()) {
        cleanup();
        resolve();
      } else {
        rafId = requestAnimationFrame(tick);
      }
    };

    rafId = requestAnimationFrame(tick);
  });
}

function isPow2(n: number): boolean {
  return n > 0 && (n & (n - 1)) === 0;
}

function abortError(): DOMException {
  return new DOMException('runWithRaf aborted', 'AbortError');
}
