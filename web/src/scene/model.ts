// The UI's copy of what the core's view shows, rebuilt from the core's
// scene updates. It makes no domain decisions: the core says what changed,
// and this applies it (replace everything, or upsert/remove by ID).
import type {
  AnnotationDto,
  EntityDto,
  IngestStatsDto,
  ProblemDto,
  SceneUpdateDto,
  TimelineDto,
} from '../generated/protocol.ts';

export interface SceneSnapshot {
  readonly entities: ReadonlyMap<string, EntityDto>;
  readonly annotations: ReadonlyMap<string, AnnotationDto>;
  readonly timeline: TimelineDto;
  readonly stats: IngestStatsDto;
  /** Set when the latest request stopped early; cleared by the next one. */
  readonly problem: ProblemDto | null;
}

function merged<T extends { id: string }>(
  previous: ReadonlyMap<string, T> | undefined,
  replace: boolean,
  upserts: readonly T[],
  removed: readonly string[],
): ReadonlyMap<string, T> {
  const next = new Map(replace ? [] : previous);
  for (const id of removed) next.delete(id);
  for (const item of upserts) next.set(item.id, item);
  return next;
}

/** The snapshot after `update`. `previous` is only read. */
export function applyUpdate(previous: SceneSnapshot | null, update: SceneUpdateDto): SceneSnapshot {
  const replace = update.replace || previous === null;
  return {
    entities: merged(previous?.entities, replace, update.entities, update.removed_entities),
    annotations: merged(
      previous?.annotations,
      replace,
      update.annotations,
      update.removed_annotations,
    ),
    timeline: update.timeline,
    stats: update.stats,
    problem: update.problem ?? null,
  };
}

/** A replacing update that recreates `snapshot`, for a renderer that
 * starts (or restarts) after the scene already has content. */
export function replacingUpdate(snapshot: SceneSnapshot): SceneUpdateDto {
  return {
    replace: true,
    timeline: snapshot.timeline,
    stats: snapshot.stats,
    entities: [...snapshot.entities.values()],
    removed_entities: [],
    annotations: [...snapshot.annotations.values()],
    removed_annotations: [],
    problem: snapshot.problem,
  };
}
