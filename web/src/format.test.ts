import { describe, expect, it } from 'vitest';
import { metres, metres1, percent, sceneTime, smallNumber } from './format.ts';

describe('format', () => {
  it('formats lengths and percentages', () => {
    expect(metres(-20_000)).toBe('-20 m');
    expect(metres(250)).toBe('0.25 m');
    expect(metres1(-12_345)).toBe('-12.3 m');
    expect(percent(8750)).toBe('87.5 %');
  });

  it('formats scene time exactly, beyond 2^53 ticks', () => {
    expect(sceneTime('0', 100)).toBe('0:00.0');
    expect(sceneTime('1', 100)).toBe('0:00.1');
    expect(sceneTime('600', 100)).toBe('1:00.0');
    expect(sceneTime('6123', 100)).toBe('10:12.3');
    // u64::MAX tenths = 600 × 30744573456182586 + 15.
    expect(sceneTime('18446744073709551615', 100)).toBe('30744573456182586:01.5');
  });

  it('refuses u64 values a Number cannot hold', () => {
    expect(smallNumber('9007199254740991')).toBe(2 ** 53 - 1);
    expect(() => smallNumber('9007199254740993')).toThrow(RangeError);
  });
});
