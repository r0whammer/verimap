import { describe, expect, it } from 'vitest';
import { FRESHNESS_LABEL } from './labels.ts';

describe('freshness labels', () => {
  it('names every freshness in words', () => {
    expect(FRESHNESS_LABEL).toEqual({
      active: 'Active',
      stale: 'Stale',
      expired: 'Expired',
    });
  });
});
