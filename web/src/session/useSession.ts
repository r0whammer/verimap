// The UI's handle on the core session: one worker for the component's
// lifetime, the scene snapshot built from the core's answers, and playback.
// Every scene change comes from the core; this hook only decides when to
// ask (playback pacing, coalescing a dragged scrubber) and keeps the answers
// in order.
import { useCallback, useEffect, useRef, useState } from 'react';
import type {
  EntityDetailDto,
  PlacedPointDto,
  TimelineDto,
  U64String,
  UiRequest,
} from '../generated/protocol.ts';
import { applyUpdate, type SceneSnapshot } from '../scene/model.ts';
import { CoreBridge, type CoreAnswer, type CoreStatus } from '../worker/bridge.ts';

export const SPEEDS = [1, 2, 5, 10] as const;
export type Speed = (typeof SPEEDS)[number];

export interface StateHash {
  readonly tick: U64String;
  readonly sequence: U64String;
  readonly hash: string;
}

export interface MarkerEdit {
  readonly label?: string;
  readonly at?: PlacedPointDto;
}

export interface Session {
  readonly core: CoreStatus;
  /** What the current view shows (live or reviewed history). */
  readonly scene: SceneSnapshot | null;
  readonly playing: boolean;
  readonly speed: Speed;
  /** The latest request failure, as text for the operator. */
  readonly notice: string | null;
  readonly stateHash: StateHash | null;
  readonly play: () => void;
  readonly pause: () => void;
  readonly setSpeed: (speed: Speed) => void;
  /** One tick forward: live, or through reviewed history. */
  readonly step: () => void;
  /** Shows history up to `tick` (read-only). Rapid calls coalesce. */
  readonly scrub: (tick: U64String) => void;
  readonly goLive: () => void;
  /** Resolves with the new marker's ID, or null if the core refused. */
  readonly addMarker: (label: string, at: PlacedPointDto) => Promise<string | null>;
  /** Resolves with whether the core accepted the edit. */
  readonly editMarker: (id: string, baseRevision: U64String, edit: MarkerEdit) => Promise<boolean>;
  readonly deleteAnnotation: (id: string, baseRevision: U64String) => Promise<boolean>;
  readonly removeEntity: (id: string, baseRevision: U64String) => Promise<boolean>;
  /** The entity's detail in the current view; no notice on failure. */
  readonly inspect: (id: string) => Promise<EntityDetailDto | null>;
  readonly computeStateHash: () => void;
  readonly dismissNotice: () => void;
}

/** A timeline request waiting to be sent. `step` is resolved to `advance`
 * or `review_step` when sent, from the mode of the latest answer. */
type TimelineIntent =
  | { readonly kind: 'step'; readonly ticks: number; readonly playback: boolean }
  | { readonly kind: 'review'; readonly tick: U64String }
  | { readonly kind: 'live' };

type Request = Exclude<UiRequest, { type: 'hello' }>;

function describe(answer: CoreAnswer & { type: 'error' }): string {
  return `${answer.message} (${answer.code.replaceAll('_', ' ')})`;
}

function atEnd(t: TimelineDto): boolean {
  return t.mode === 'review' && t.tick === t.live_tick;
}

export function useSession(): Session {
  const [core, setCore] = useState<CoreStatus>({ state: 'starting' });
  const [scene, setScene] = useState<SceneSnapshot | null>(null);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState<Speed>(1);
  const [notice, setNotice] = useState<string | null>(null);
  const [stateHash, setStateHash] = useState<StateHash | null>(null);
  const bridge = useRef<CoreBridge | null>(null);
  // The timeline of the latest answer, read when sending the next timeline
  // request (React state may not have caught up yet).
  const timeline = useRef<TimelineDto | null>(null);
  // Timeline requests go one at a time, so a mode change (review/live) is
  // always answered before the next step is resolved against it.
  const queue = useRef<TimelineIntent[]>([]);
  const timelineBusy = useRef(false);

  useEffect(() => {
    const b = new CoreBridge();
    bridge.current = b;
    const unsubscribe = b.subscribe((status) => {
      setCore(status);
      if (status.state === 'ready') {
        timeline.current = status.initialScene.timeline;
        setScene(applyUpdate(null, status.initialScene));
      } else {
        setPlaying(false);
      }
    });
    return () => {
      unsubscribe();
      b.dispose();
      bridge.current = null;
      queue.current = [];
      timelineBusy.current = false;
    };
  }, []);

  const send = useCallback(async (request: Request, quiet = false): Promise<CoreAnswer> => {
    const b = bridge.current;
    if (!b) return { type: 'error', code: 'internal', message: 'core stopped' };
    const answer = await b.request(request);
    if (answer.type === 'scene') {
      const { update } = answer;
      timeline.current = update.timeline;
      setScene((previous) => applyUpdate(previous, update));
      if (update.problem) {
        setNotice(`${update.problem.message} (${update.problem.code.replaceAll('_', ' ')})`);
        setPlaying(false);
      }
    } else if (answer.type === 'error' && !quiet) {
      setNotice(describe(answer));
    }
    return answer;
  }, []);

  // Sends queued timeline requests one at a time until the queue is empty.
  const pump = useCallback(() => {
    const next = (): void => {
      if (timelineBusy.current) return;
      const intent = queue.current.shift();
      if (!intent) return;
      let request: Request;
      if (intent.kind === 'review') request = { type: 'review', tick: intent.tick };
      else if (intent.kind === 'live') request = { type: 'go_live' };
      else if (timeline.current?.mode === 'review') {
        if (atEnd(timeline.current)) {
          // Nothing recorded beyond this point.
          if (intent.playback) setPlaying(false);
          next();
          return;
        }
        request = { type: 'review_step', ticks: intent.ticks };
      } else {
        request = { type: 'advance', ticks: intent.ticks };
      }
      timelineBusy.current = true;
      void send(request).then((answer) => {
        timelineBusy.current = false;
        if (intent.kind === 'step' && intent.playback) {
          if (answer.type !== 'scene' || atEnd(answer.update.timeline)) setPlaying(false);
        }
        next();
      });
    };
    next();
  }, [send]);

  const enqueue = useCallback(
    (intent: TimelineIntent) => {
      // A newer scrub target replaces one not yet sent.
      if (intent.kind === 'review') {
        queue.current = queue.current.filter((i) => i.kind !== 'review');
      }
      queue.current.push(intent);
      pump();
    },
    [pump],
  );

  const ready = core.state === 'ready';
  const tickMs = ready ? core.session.tick_ms : 100;

  // Playback: one request per tick interval, `speed` ticks each, skipped
  // while the previous one is unanswered. A slow core therefore plays
  // slower than nominal rather than building a backlog.
  useEffect(() => {
    if (!playing || !ready) return;
    const id = setInterval(() => {
      if (timelineBusy.current || queue.current.length > 0) return;
      enqueue({ kind: 'step', ticks: speed, playback: true });
    }, tickMs);
    return () => {
      clearInterval(id);
    };
  }, [playing, ready, speed, tickMs, enqueue]);

  const mutate = useCallback(
    async (request: Request): Promise<CoreAnswer> => {
      const answer = await send(request);
      if (answer.type === 'scene') setNotice(null);
      return answer;
    },
    [send],
  );

  return {
    core,
    scene,
    playing,
    speed,
    notice,
    stateHash,
    play: useCallback(() => {
      setNotice(null);
      setPlaying(true);
    }, []),
    pause: useCallback(() => {
      setPlaying(false);
    }, []),
    setSpeed,
    step: useCallback(() => {
      enqueue({ kind: 'step', ticks: 1, playback: false });
    }, [enqueue]),
    scrub: useCallback(
      (tick: U64String) => {
        enqueue({ kind: 'review', tick });
      },
      [enqueue],
    ),
    goLive: useCallback(() => {
      enqueue({ kind: 'live' });
    }, [enqueue]),
    addMarker: useCallback(
      async (label: string, at: PlacedPointDto) => {
        const answer = await mutate({ type: 'add_marker', label, at });
        return answer.type === 'scene' ? (answer.update.annotations[0]?.id ?? null) : null;
      },
      [mutate],
    ),
    editMarker: useCallback(
      async (id: string, baseRevision: U64String, edit: MarkerEdit) => {
        const answer = await mutate({
          type: 'edit_marker',
          id,
          base_revision: baseRevision,
          label: edit.label ?? null,
          at: edit.at ?? null,
        });
        return answer.type === 'scene';
      },
      [mutate],
    ),
    deleteAnnotation: useCallback(
      async (id: string, baseRevision: U64String) =>
        (await mutate({ type: 'delete_annotation', id, base_revision: baseRevision })).type ===
        'scene',
      [mutate],
    ),
    removeEntity: useCallback(
      async (id: string, baseRevision: U64String) =>
        (await mutate({ type: 'remove_entity', id, base_revision: baseRevision })).type === 'scene',
      [mutate],
    ),
    inspect: useCallback(
      async (id: string) => {
        const answer = await send({ type: 'inspect_entity', id }, true);
        return answer.type === 'entity_detail' ? answer.detail : null;
      },
      [send],
    ),
    computeStateHash: useCallback(() => {
      void send({ type: 'state_hash' }).then((answer) => {
        if (answer.type === 'state_hash') {
          setStateHash({ tick: answer.tick, sequence: answer.sequence, hash: answer.hash });
        }
      });
    }, [send]),
    dismissNotice: useCallback(() => {
      setNotice(null);
    }, []),
  };
}

/** The selected entity's history in the current view, refreshed whenever
 * its revision in the view changes. Null while unknown or not in view;
 * while a refresh is pending, the previous answer for the same entity. */
export function useEntityDetail(session: Session, id: string | null): EntityDetailDto | null {
  const [latest, setLatest] = useState<EntityDetailDto | null>(null);
  const revision = id === null ? undefined : session.scene?.entities.get(id)?.revision;
  const { inspect } = session;
  useEffect(() => {
    if (id === null || revision === undefined) return;
    let current = true;
    void inspect(id).then((d) => {
      if (current) setLatest(d);
    });
    return () => {
      current = false;
    };
  }, [id, revision, inspect]);
  return revision !== undefined && latest?.entity.id === id ? latest : null;
}
