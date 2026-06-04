import { describe, expect, it } from 'vitest';
import { getKeySnapshot } from './browser';

describe('getKeySnapshot', () => {
  it('normalizes plain keys', () => {
    expect(getKeySnapshot(keyboardEvent('Enter'))).toBe('enter');
    expect(getKeySnapshot(keyboardEvent(' '))).toBe('space');
    expect(getKeySnapshot(keyboardEvent('ArrowLeft'))).toBe('arrow-left');
  });

  it('includes modifiers in stable order', () => {
    expect(getKeySnapshot(keyboardEvent('Enter', { ctrlKey: true }))).toBe(
      'ctrl-enter',
    );
    expect(
      getKeySnapshot(keyboardEvent('Enter', { ctrlKey: true, metaKey: true })),
    ).toBe('meta-ctrl-enter');
    expect(
      getKeySnapshot(
        keyboardEvent('ArrowDown', {
          altKey: true,
          ctrlKey: true,
          metaKey: true,
          shiftKey: true,
        }),
      ),
    ).toBe('meta-ctrl-alt-shift-arrow-down');
  });
});

const keyboardEvent = (
  key: string,
  modifiers: Partial<
    Pick<KeyboardEvent, 'altKey' | 'ctrlKey' | 'metaKey' | 'shiftKey'>
  > = {},
): KeyboardEvent =>
  ({
    key,
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    ...modifiers,
  }) as KeyboardEvent;
