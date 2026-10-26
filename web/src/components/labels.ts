import type { FreshnessDto } from '../generated/protocol.ts';

export const FRESHNESS_LABEL: Record<FreshnessDto, string> = {
  active: 'Active',
  stale: 'Stale',
  expired: 'Expired',
};
