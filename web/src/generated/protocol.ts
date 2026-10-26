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
      site: SiteDto;
      type: 'ready';
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
 * Why a request was rejected.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "ErrorCode".
 */
export type ErrorCode = 'request_too_large' | 'malformed_request' | 'invalid_field' | 'internal';
/**
 * A request from the UI thread.
 *
 * This interface was referenced by `WorkerBridge`'s JSON-Schema
 * via the `definition` "UiRequest".
 */
export type UiRequest = {
  protocol: ProtocolVersion;
  type: 'hello';
  /**
   * UI build identifier, for diagnostics only.
   */
  ui_build: string;
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

/** Protocol version of the Rust core these types were generated from. */
export const PROTOCOL_VERSION: ProtocolVersion = { major: 0, minor: 1 };
