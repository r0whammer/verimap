import { describe, expect, it } from 'vitest';
import type { AnnotationDto, EntityDto, SceneUpdateDto } from '../generated/protocol.ts';
import { applyUpdate, replacingUpdate } from './model.ts';

function entity(id: string, x_mm: number): EntityDto {
  return {
    id,
    class: 'vehicle',
    position: { x_mm, y_mm: 0, z_mm: 0 },
    sigma_mm: 1500,
    confidence_bp: 9000,
    freshness: 'active',
    last_observed_tick: '1',
    revision: '1',
    source: { source_id: 'sim-detector-1', sequence: '1' },
  };
}

function marker(id: string, label: string): AnnotationDto {
  return {
    id,
    label,
    position: { x_mm: 0, y_mm: 0, z_mm: 0 },
    placement: 'ground_plane',
    revision: '2',
  };
}

function update(partial: Partial<SceneUpdateDto>): SceneUpdateDto {
  return {
    replace: false,
    timeline: {
      mode: 'live',
      tick: '1',
      sequence: '1',
      live_tick: '1',
      live_sequence: '1',
      journal_events: '1',
      journal_limit: '10',
    },
    stats: { received: '0', accepted: '0', rejected: '0', duplicates: '0', late: '0' },
    entities: [],
    removed_entities: [],
    annotations: [],
    removed_annotations: [],
    problem: null,
    ...partial,
  };
}

describe('applyUpdate', () => {
  it('upserts and removes by ID', () => {
    let s = applyUpdate(null, update({ replace: true, entities: [entity('trk:1', 0)] }));
    s = applyUpdate(s, update({ entities: [entity('trk:1', 5), entity('trk:2', 9)] }));
    expect([...s.entities.keys()]).toEqual(['trk:1', 'trk:2']);
    expect(s.entities.get('trk:1')?.position.x_mm).toBe(5);
    s = applyUpdate(
      s,
      update({ removed_entities: ['trk:1'], annotations: [marker('op:1', 'Gate')] }),
    );
    expect([...s.entities.keys()]).toEqual(['trk:2']);
    expect(s.annotations.get('op:1')?.label).toBe('Gate');
  });

  it('replace discards everything not in the update', () => {
    let s = applyUpdate(null, update({ entities: [entity('trk:1', 0)] }));
    s = applyUpdate(s, update({ replace: true, annotations: [marker('op:1', 'A')] }));
    expect(s.entities.size).toBe(0);
    expect(s.annotations.size).toBe(1);
  });

  it('never mutates the previous snapshot', () => {
    const a = applyUpdate(null, update({ entities: [entity('trk:1', 0)] }));
    const b = applyUpdate(a, update({ removed_entities: ['trk:1'] }));
    expect(a.entities.size).toBe(1);
    expect(b.entities.size).toBe(0);
  });

  it('replacingUpdate recreates the snapshot', () => {
    const a = applyUpdate(
      null,
      update({ entities: [entity('trk:1', 3)], annotations: [marker('op:1', 'A')] }),
    );
    const b = applyUpdate(
      applyUpdate(null, update({ entities: [entity('x:9', 1)] })),
      replacingUpdate(a),
    );
    expect(b).toEqual(a);
  });
});
