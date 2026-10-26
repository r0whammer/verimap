// Generated from schemas/worker-bridge.schema.json by web/scripts/gen-types.mjs.
// Do not edit; run `just gen`. Types only: incoming data is validated in Rust.

/**
 * The core's answer to one request.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "CoreEvent".
 */
export type CoreEvent =
  | {
      /**
       * Core (WASM) build identifier, for diagnostics only.
       */
      core_build: string;
      protocol: ProtocolVersion;
      scene: SceneUpdateDto;
      session: SessionDto;
      site: SiteDto;
      type: 'ready';
    }
  | {
      type: 'scene';
      update: SceneUpdateDto;
    }
  | {
      detail: EntityDetailDto;
      type: 'entity_detail';
    }
  | {
      /**
       * 64-bit FNV-1a of the canonical encoding, 16 lowercase hex digits.
       */
      hash: string;
      sequence: U64String;
      tick: U64String;
      type: 'state_hash';
    }
  | {
      core_protocol: ProtocolVersion;
      type: 'incompatible';
      ui_protocol: ProtocolVersion;
    }
  | {
      code: ErrorCode;
      /**
       * Human-readable detail. Render as text only.
       */
      message: string;
      type: 'error';
    };
/**
 * Where an annotation's height came from.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "PlacementDto".
 */
export type PlacementDto = 'ground_plane';
/**
 * A u64 as a canonical decimal string (no sign or leading zeros). Never convert it to a JavaScript Number unless it is known to be below 2^53.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "U64String".
 */
export type U64String = string;
/**
 * Believed class of a track: detector output, not a verified identity.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "EntityClassDto".
 */
export type EntityClassDto =
  'person' | 'animal' | 'vehicle' | 'building' | 'obstacle' | 'sensor' | 'unknown';
/**
 * How recently a track was observed, by the scene clock.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "FreshnessDto".
 */
export type FreshnessDto = 'active' | 'stale' | 'expired';
/**
 * Why a request was rejected.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "ErrorCode".
 */
export type ErrorCode =
  | 'request_too_large'
  | 'malformed_request'
  | 'invalid_field'
  | 'handshake_required'
  | 'not_found'
  | 'deleted'
  | 'precondition_failed'
  | 'wrong_mode'
  | 'journal_full'
  | 'rejected'
  | 'internal';
/**
 * Whether the view shows the live session or a replayed past state.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "ViewModeDto".
 */
export type ViewModeDto = 'live' | 'review';
/**
 * A request from the UI thread.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "UiRequest".
 */
export type UiRequest =
  | {
      protocol: ProtocolVersion;
      type: 'hello';
      /**
       * UI build identifier, for diagnostics only.
       */
      ui_build: string;
    }
  | {
      /**
       * Scene ticks to simulate.
       */
      ticks: number;
      type: 'advance';
    }
  | {
      at: PlacedPointDto;
      /**
       * Label, 1–128 bytes of UTF-8 without control characters.
       */
      label: string;
      type: 'add_marker';
    }
  | {
      /**
       * New position.
       */
      at?: PlacedPointDto | null;
      base_revision: U64String;
      /**
       * Annotation ID.
       */
      id: string;
      /**
       * New label.
       */
      label?: string | null;
      type: 'edit_marker';
    }
  | {
      base_revision: U64String;
      /**
       * Annotation ID.
       */
      id: string;
      type: 'delete_annotation';
    }
  | {
      base_revision: U64String;
      /**
       * Track ID.
       */
      id: string;
      type: 'remove_entity';
    }
  | {
      /**
       * Track ID.
       */
      id: string;
      type: 'inspect_entity';
    }
  | {
      tick: U64String;
      type: 'review';
    }
  | {
      /**
       * Scene ticks to move.
       */
      ticks: number;
      type: 'review_step';
    }
  | {
      type: 'go_live';
    }
  | {
      type: 'state_hash';
    };

/**
 * Schema root naming every bridge message type, so generated TypeScript
 * declares each exactly once. Never sent on the wire.
 */
export interface WorkerBridge {
  event: CoreEvent;
  request: UiRequest;
}
/**
 * A bridge protocol version. See docs/protocol.md for when each part changes.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "ProtocolVersion".
 */
export interface ProtocolVersion {
  /**
   * Incompatible changes.
   */
  major: number;
  /**
   * Backward-compatible additions.
   */
  minor: number;
}
/**
 * A change to what the view shows.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "SceneUpdateDto".
 */
export interface SceneUpdateDto {
  /**
   * Annotations added or changed.
   */
  annotations: AnnotationDto[];
  /**
   * Tracks added or changed.
   */
  entities: EntityDto[];
  /**
   * Why the request stopped early, when the changes above are only part
   * of what was asked (for example a full journal stops the simulation).
   */
  problem?: ProblemDto | null;
  /**
   * Annotation IDs no longer present.
   */
  removed_annotations: string[];
  /**
   * Track IDs no longer present (deleted, or absent in a review state).
   */
  removed_entities: string[];
  /**
   * When true, the lists below are the complete view: discard
   * everything else. When false, they are changes to apply.
   */
  replace: boolean;
  stats: IngestStatsDto;
  timeline: TimelineDto;
}
/**
 * An operator annotation.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "AnnotationDto".
 */
export interface AnnotationDto {
  /**
   * Stable ID, `namespace:local`.
   */
  id: string;
  /**
   * Label. Untrusted: render as text only.
   */
  label: string;
  placement: PlacementDto;
  position: PointMmDto;
  revision: U64String;
}
/**
 * A scene position in integer millimetres (ENU from the anchor). Values
 * are within ±10⁸, so they are exact as JSON numbers.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "PointMmDto".
 */
export interface PointMmDto {
  /**
   * East.
   */
  x_mm: number;
  /**
   * North.
   */
  y_mm: number;
  /**
   * Up.
   */
  z_mm: number;
}
/**
 * A track as the map shows it.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "EntityDto".
 */
export interface EntityDto {
  class: EntityClassDto;
  /**
   * Detector confidence, basis points (0–10000). Not a calibrated
   * probability.
   */
  confidence_bp: number;
  freshness: FreshnessDto;
  /**
   * Stable track ID, `namespace:local`. Not a real-world identity.
   */
  id: string;
  last_observed_tick: U64String;
  position: PointMmDto;
  revision: U64String;
  /**
   * Horizontal 1σ uncertainty, millimetres.
   */
  sigma_mm: number;
  source: SourceRefDto;
}
/**
 * The observation a state came from.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "SourceRefDto".
 */
export interface SourceRefDto {
  sequence: U64String;
  /**
   * Reporting source.
   */
  source_id: string;
}
/**
 * A problem reported alongside a partial result.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "ProblemDto".
 */
export interface ProblemDto {
  code: ErrorCode;
  /**
   * Human-readable detail. Render as text only.
   */
  message: string;
}
/**
 * Observation counts since the session started.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "IngestStatsDto".
 */
export interface IngestStatsDto {
  accepted: U64String;
  duplicates: U64String;
  late: U64String;
  received: U64String;
  rejected: U64String;
}
/**
 * Where the view is in the session.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "TimelineDto".
 */
export interface TimelineDto {
  journal_events: U64String;
  journal_limit: U64String;
  live_sequence: U64String;
  live_tick: U64String;
  mode: ViewModeDto;
  sequence: U64String;
  tick: U64String;
}
/**
 * Versions and parameters of the running session.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "SessionDto".
 */
export interface SessionDto {
  /**
   * Committed-event format and reducer version.
   */
  event_schema_version: number;
  /**
   * Ticks without observation before a track is expired.
   */
  expire_after_ticks: number;
  /**
   * Synthetic scene version.
   */
  scene_version: number;
  seed: U64String;
  /**
   * Simulator behavior version.
   */
  simulator_version: number;
  /**
   * Ticks without observation before a track is stale.
   */
  stale_after_ticks: number;
  /**
   * Scene time per tick, milliseconds.
   */
  tick_ms: number;
}
/**
 * A site's identity, anchor, and scene bounds (docs/coordinates.md).
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "SiteDto".
 */
export interface SiteDto {
  anchor: GeodeticAnchorDto;
  bounds: SceneBoundsDto;
  /**
   * Display name, at most 128 bytes. Untrusted: render as text only.
   */
  name: string;
  /**
   * Lowercase slug, at most 64 bytes.
   */
  site_id: string;
}
/**
 * A WGS84 position. Height is above the ellipsoid, not mean sea level.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "GeodeticAnchorDto".
 */
export interface GeodeticAnchorDto {
  /**
   * Metres above the WGS84 ellipsoid.
   */
  ellipsoidal_height_m: number;
  /**
   * Degrees, north positive; sites are limited to ±85°.
   */
  latitude_deg: number;
  /**
   * Degrees, east positive.
   */
  longitude_deg: number;
}
/**
 * Per-axis scene bounds in millimetres: x east, y north, z up.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "SceneBoundsDto".
 */
export interface SceneBoundsDto {
  x: AxisBoundsDto;
  y: AxisBoundsDto;
  z: AxisBoundsDto;
}
/**
 * A closed interval of scene coordinates in integer millimetres.
 *
 * Values are within ±10⁸, so they are exact as JSON numbers in JavaScript.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "AxisBoundsDto".
 */
export interface AxisBoundsDto {
  /**
   * Upper end, inclusive.
   */
  max_mm: number;
  /**
   * Lower end, inclusive.
   */
  min_mm: number;
}
/**
 * A track with its observation history, for the detail panel.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "EntityDetailDto".
 */
export interface EntityDetailDto {
  entity: EntityDto;
  /**
   * Most recent fixes, oldest first (at most 16).
   */
  history: FixDto[];
}
/**
 * One past fix in a track's history.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "FixDto".
 */
export interface FixDto {
  acquired_tick: U64String;
  /**
   * Detector confidence, basis points.
   */
  confidence_bp: number;
  position: PointMmDto;
  revision: U64String;
  /**
   * Horizontal 1σ uncertainty, millimetres.
   */
  sigma_mm: number;
  source: SourceRefDto;
}
/**
 * A picked position and where its height came from.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "PlacedPointDto".
 */
export interface PlacedPointDto {
  placement: PlacementDto;
  position: PointMDto;
}
/**
 * A position the UI picked, in scene-frame metres. The core quantizes it
 * to millimetres and validates it against the site.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "PointMDto".
 */
export interface PointMDto {
  /**
   * East, metres.
   */
  x_m: number;
  /**
   * North, metres.
   */
  y_m: number;
  /**
   * Up, metres.
   */
  z_m: number;
}

/** Protocol version of the Rust core these types were generated from. */
export const PROTOCOL_VERSION: ProtocolVersion = { major: 0, minor: 2 };
