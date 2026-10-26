// The Cesium adapter: the only code that creates, updates, or disposes
// Cesium objects. React renders controls around it and calls these methods;
// it never owns render objects. Scene content arrives from the Rust core as
// DTOs; this module only places it for rendering.
import 'cesium/Build/Cesium/Widgets/CesiumWidget/CesiumWidget.css';
import {
  Billboard,
  BillboardCollection,
  BoundingSphere,
  Cartesian2,
  Cartesian3,
  Cartographic,
  CesiumWidget,
  Color,
  HeadingPitchRange,
  ImageryLayer,
  Label,
  LabelCollection,
  LabelStyle,
  Material,
  Math as CesiumMath,
  Matrix4,
  PointPrimitive,
  PointPrimitiveCollection,
  Polyline,
  PolylineCollection,
  Rectangle,
  SceneMode,
  ScreenSpaceEventHandler,
  ScreenSpaceEventType,
  SingleTileImageryProvider,
  Transforms,
  VerticalOrigin,
  WebMercatorProjection,
} from 'cesium';
import type {
  AnnotationDto,
  EntityClassDto,
  EntityDto,
  FreshnessDto,
  PointMmDto,
  SiteDto,
} from '../generated/protocol.ts';
import { markerPinImage } from './icons.ts';
import { GROUND_COLOR, siteGroundImage, solidColorImage } from './siteGround.ts';

export type ViewMode = '2d' | '3d';

/** A selectable scene object. */
export interface Pick {
  readonly kind: 'entity' | 'annotation';
  readonly id: string;
}

/** What a click or tap on the map does. */
export type Tool = 'navigate' | 'place';

export interface MapEvents {
  /** A 2D/3D switch finished; `mode` is the mode now shown. */
  onModeChange: (mode: ViewMode) => void;
  /** The renderer hit a problem. The core session is unaffected. */
  onRendererProblem: (message: string) => void;
  /** A click/tap in navigate mode: the object under it, or null. */
  onPick: (pick: Pick | null) => void;
  /** A click/tap in place mode: the ground-plane point under it in scene
   * metres (x east, y north; z = 0 by construction), or null when the
   * click did not hit the ground. */
  onPlace: (point: { x_m: number; y_m: number } | null) => void;
}

const MORPH_SECONDS = 0.6;
const VIEW_PITCH = CesiumMath.toRadians(-40);
/** Camera range used to fit a bounding sphere, in sphere radii. */
const FIT_RANGE_RADII = 2.5;
/** Camera range when centring on one object, metres. */
const CENTER_RANGE_M = 120;
const RING_SEGMENTS = 48;

const CLASS_COLOR: Record<EntityClassDto, string> = {
  vehicle: '#58a6ff',
  person: '#f778ba',
  animal: '#3fb950',
  building: '#a5a5a5',
  obstacle: '#d29922',
  sensor: '#bc8cff',
  unknown: '#ffffff',
};

export const CLASS_LABEL: Record<EntityClassDto, string> = {
  vehicle: 'Vehicle',
  person: 'Person',
  animal: 'Animal',
  building: 'Building',
  obstacle: 'Obstacle',
  sensor: 'Sensor',
  unknown: 'Unknown',
};

const FRESHNESS_ALPHA: Record<FreshnessDto, number> = { active: 1, stale: 0.55, expired: 0.3 };
const SELECTED_OUTLINE = Color.fromCssColorString('#e8c547');
const MARKER_COLOR = '#39c5cf';
const PREVIEW_COLOR = '#ffffff';

interface SiteLayer {
  readonly ground: ImageryLayer;
  readonly lines: PolylineCollection;
  readonly points: PointPrimitiveCollection;
  readonly labels: LabelCollection;
  readonly sphere: BoundingSphere;
}

/** The scene frame: ENU at the anchor, in metres. */
interface Frame {
  readonly enuToFixed: Matrix4;
  readonly fixedToEnu: Matrix4;
}

interface EntityGraphics {
  dto: EntityDto;
  readonly point: PointPrimitive;
  readonly label: Label;
  readonly ring: Polyline;
}

interface MarkerGraphics {
  dto: AnnotationDto;
  readonly pin: Billboard;
  readonly label: Label;
}

/** A view to restore after a 2D/3D switch: what was centred, from how far. */
interface ViewOfInterest {
  readonly center: Cartesian3;
  readonly range: number;
}

export class CesiumMap {
  readonly #widget: CesiumWidget;
  readonly #events: MapEvents;
  readonly #input: ScreenSpaceEventHandler;
  #site: SiteLayer | null = null;
  #frame: Frame | null = null;
  #pendingView: ViewOfInterest | null = null;
  #tool: Tool = 'navigate';
  #selection: Pick | null = null;
  // Scene content, keyed by object ID. Collections are created once and
  // reused; individual primitives are added and removed as objects come
  // and go, so stable IDs keep stable render objects.
  readonly #entityPoints = new PointPrimitiveCollection();
  readonly #entityLabels = new LabelCollection();
  readonly #rings = new PolylineCollection();
  readonly #markerPins = new BillboardCollection();
  readonly #markerLabels = new LabelCollection();
  readonly #entities = new Map<string, EntityGraphics>();
  readonly #markers = new Map<string, MarkerGraphics>();
  readonly #pinImage = markerPinImage(MARKER_COLOR);
  readonly #previewImage = markerPinImage(PREVIEW_COLOR);
  #preview: Billboard | null = null;

  constructor(container: HTMLElement, events: MapEvents) {
    this.#events = events;
    // Cesium's default base layer is a Cesium ion service; use a locally
    // generated plain colour instead. A real base layer is also needed
    // because Cesium stretches the bottom layer's edges over the whole globe,
    // which would smear the site ground everywhere.
    const baseLayer = ImageryLayer.fromProviderAsync(
      SingleTileImageryProvider.fromUrl(solidColorImage(GROUND_COLOR), {
        rectangle: Rectangle.MAX_VALUE,
      }),
    );
    this.#widget = new CesiumWidget(container, {
      baseLayer,
      // Conformal 2D view: a square site stays square. Cesium's default
      // (plate carrée) stretches east-west by 1/cos(latitude), 1.28x at the
      // demo site. Valid to ±85.05°, matching the domain's ±85° anchor limit.
      mapProjection: new WebMercatorProjection(),
      skyBox: false,
      skyAtmosphere: false,
      requestRenderMode: true,
      maximumRenderTimeChange: Infinity,
      showRenderLoopErrors: false,
    });
    const scene = this.#widget.scene;
    // Imagery loads asynchronously; in request-render mode it must ask for a
    // frame when it arrives, which may be after destroy().
    const renderWhenReady = (layer: ImageryLayer) => {
      layer.readyEvent.addEventListener(this.#requestRender);
    };
    renderWhenReady(baseLayer);
    scene.imageryLayers.layerAdded.addEventListener(renderWhenReady);
    scene.globe.baseColor = Color.fromCssColorString(GROUND_COLOR);
    scene.globe.showGroundAtmosphere = false;
    scene.backgroundColor = Color.fromCssColorString('#0d1117');
    scene.fog.enabled = false;
    scene.screenSpaceCameraController.minimumZoomDistance = 5;
    for (const collection of [
      this.#rings,
      this.#entityPoints,
      this.#entityLabels,
      this.#markerPins,
      this.#markerLabels,
    ]) {
      scene.primitives.add(collection);
    }
    scene.renderError.addEventListener(this.#onRenderError);
    scene.morphComplete.addEventListener(this.#onMorphComplete);
    scene.canvas.addEventListener('webglcontextlost', this.#onContextLost);
    // Cesium reports a click only when the pointer did not move, so a drag
    // always pans and never places or selects.
    this.#input = new ScreenSpaceEventHandler(scene.canvas);
    this.#input.setInputAction(this.#onClick, ScreenSpaceEventType.LEFT_CLICK);
  }

  /** The mode shown, or the target mode while a switch is in progress. */
  get mode(): ViewMode {
    return this.#widget.scene.mode === SceneMode.SCENE2D ? '2d' : '3d';
  }

  get switching(): boolean {
    return this.#widget.scene.mode === SceneMode.MORPHING;
  }

  /** Replaces the drawn site: reference ground, scene-bounds box, anchor.
   * Scene content is cleared; `sync` redraws it in the new frame. */
  setSite(site: SiteDto): void {
    this.#clearSite();
    this.#clearContent();
    const scene = this.#widget.scene;
    const { anchor, bounds } = site;
    const { x, y, z } = bounds;
    const origin = Cartesian3.fromDegrees(
      anchor.longitude_deg,
      anchor.latitude_deg,
      anchor.ellipsoidal_height_m,
    );
    const enuToFixed = Transforms.eastNorthUpToFixedFrame(origin);
    this.#frame = {
      enuToFixed,
      fixedToEnu: Matrix4.inverseTransformation(enuToFixed, new Matrix4()),
    };
    const at = (eMm: number, nMm: number, uMm: number) => this.#fixed(eMm, nMm, uMm);

    // The site's horizontal corners at height `uMm`, counter-clockwise from
    // south-west.
    const ring = (uMm: number) => [
      at(x.min_mm, y.min_mm, uMm),
      at(x.max_mm, y.min_mm, uMm),
      at(x.max_mm, y.max_mm, uMm),
      at(x.min_mm, y.max_mm, uMm),
    ];

    // Reference ground: imagery draped on the ellipsoid, so it cannot
    // z-fight with the globe. Over a site this small the ENU square and its
    // longitude/latitude rectangle differ by millimetres.
    const footprint = ring(0);
    const ground = ImageryLayer.fromProviderAsync(
      SingleTileImageryProvider.fromUrl(siteGroundImage(bounds), {
        rectangle: Rectangle.fromCartographicArray(
          footprint.map((p) => Cartographic.fromCartesian(p)),
        ),
      }),
    );
    ground.errorEvent.addEventListener((error: unknown) => {
      if (!this.#widget.isDestroyed())
        this.#events.onRendererProblem(`site ground failed: ${String(error)}`);
    });
    scene.imageryLayers.add(ground);

    // Scene-bounds box: the volume scene content may occupy.
    const lines = scene.primitives.add(new PolylineCollection());
    const boxMaterial = Material.fromType('Color', {
      color: Color.fromCssColorString('#e8c547').withAlpha(0.8),
    });
    const low = ring(z.min_mm);
    const high = ring(z.max_mm);
    for (const ring of [low, high]) {
      lines.add({ positions: [...ring, ...ring.slice(0, 1)], width: 2, material: boxMaterial });
    }
    low.forEach((p, i) => {
      const q = high[i];
      if (q) lines.add({ positions: [p, q], width: 2, material: boxMaterial });
    });

    // Anchor: the scene frame's origin, always visible.
    const points = scene.primitives.add(new PointPrimitiveCollection());
    points.add({
      position: origin,
      pixelSize: 10,
      color: Color.WHITE,
      outlineColor: Color.BLACK,
      outlineWidth: 2,
      disableDepthTestDistance: Number.POSITIVE_INFINITY,
    });
    const labels = scene.primitives.add(new LabelCollection());
    labels.add({
      position: origin,
      text: 'Anchor (0, 0, 0)',
      font: '14px system-ui, sans-serif',
      fillColor: Color.WHITE,
      outlineColor: Color.BLACK,
      outlineWidth: 3,
      style: LabelStyle.FILL_AND_OUTLINE,
      verticalOrigin: VerticalOrigin.BOTTOM,
      pixelOffset: new Cartesian2(0, -12),
      disableDepthTestDistance: Number.POSITIVE_INFINITY,
    });

    this.#site = {
      ground,
      lines,
      points,
      labels,
      sphere: BoundingSphere.fromPoints([...low, ...high]),
    };
    scene.requestRender();
  }

  /** Makes the drawn tracks and markers match these maps. Objects whose
   * DTO is the same object as last time are left alone, so only changed
   * objects are touched. Requires `setSite` first. */
  sync(
    entities: ReadonlyMap<string, EntityDto>,
    annotations: ReadonlyMap<string, AnnotationDto>,
  ): void {
    if (!this.#frame) return;
    for (const [id, g] of this.#entities) {
      if (!entities.has(id)) {
        this.#entityPoints.remove(g.point);
        this.#entityLabels.remove(g.label);
        this.#rings.remove(g.ring);
        this.#entities.delete(id);
      }
    }
    for (const [id, dto] of entities) {
      const g = this.#entities.get(id);
      if (!g) this.#entities.set(id, this.#addEntity(dto));
      else if (g.dto !== dto) this.#updateEntity(g, dto);
    }
    for (const [id, g] of this.#markers) {
      if (!annotations.has(id)) {
        this.#markerPins.remove(g.pin);
        this.#markerLabels.remove(g.label);
        this.#markers.delete(id);
      }
    }
    for (const [id, dto] of annotations) {
      const g = this.#markers.get(id);
      if (!g) this.#markers.set(id, this.#addMarker(dto));
      else if (g.dto !== dto) this.#updateMarker(g, dto);
    }
    this.#restyleSelection();
    this.#widget.scene.requestRender();
  }

  /** Highlights `pick` (or nothing). */
  setSelection(pick: Pick | null): void {
    this.#selection = pick;
    this.#restyleSelection();
    this.#widget.scene.requestRender();
  }

  /** What a click or tap does from now on. */
  setTool(tool: Tool): void {
    this.#tool = tool;
    this.#widget.scene.canvas.style.cursor = tool === 'place' ? 'crosshair' : '';
  }

  /** Shows an uncommitted placement (scene metres on the ground plane), or
   * hides it. The preview is not pickable. */
  setPreview(point: { x_m: number; y_m: number } | null): void {
    if (this.#preview) {
      this.#markerPins.remove(this.#preview);
      this.#preview = null;
    }
    if (point && this.#frame) {
      this.#preview = this.#markerPins.add({
        position: this.#fixed(point.x_m * 1000, point.y_m * 1000, 0),
        image: this.#previewImage,
        color: Color.WHITE.withAlpha(0.85),
        verticalOrigin: VerticalOrigin.BOTTOM,
        disableDepthTestDistance: Number.POSITIVE_INFINITY,
      });
    }
    this.#widget.scene.requestRender();
  }

  /** Centres the view on a track or marker, keeping the heading. */
  centerOn(pick: Pick): void {
    const p =
      pick.kind === 'entity'
        ? this.#entities.get(pick.id)?.point.position
        : this.#markers.get(pick.id)?.pin.position;
    if (!p) return;
    const camera = this.#widget.camera;
    camera.flyToBoundingSphere(new BoundingSphere(p, 1), {
      duration: 0,
      offset: new HeadingPitchRange(camera.heading, VIEW_PITCH, CENTER_RANGE_M),
    });
    this.#widget.scene.requestRender();
  }

  /** Frames the whole site (the "home" view). */
  fitSite(): void {
    if (this.#site)
      this.#lookAt(this.#site.sphere.center, this.#site.sphere.radius * FIT_RANGE_RADII);
  }

  /** Turns the 3D view to face north, orbiting the view centre. The 2D view
   * is always north-up. */
  northUp(): void {
    const scene = this.#widget.scene;
    if (scene.mode !== SceneMode.SCENE3D) return;
    const view = this.#captureView();
    const camera = this.#widget.camera;
    if (view) {
      camera.lookAt(view.center, new HeadingPitchRange(0, camera.pitch, view.range));
      camera.lookAtTransform(Matrix4.IDENTITY);
    } else {
      camera.setView({ orientation: { heading: 0, pitch: camera.pitch, roll: 0 } });
    }
    scene.requestRender();
  }

  /** Starts a switch to `mode`; returns false if none was started. The area
   * in view is restored when the switch completes. */
  setMode(mode: ViewMode): boolean {
    const scene = this.#widget.scene;
    if (this.switching || mode === this.mode) return false;
    this.#pendingView = this.#captureView();
    if (mode === '2d') scene.morphTo2D(MORPH_SECONDS);
    else scene.morphTo3D(MORPH_SECONDS);
    scene.requestRender();
    return true;
  }

  destroy(): void {
    const scene = this.#widget.scene;
    this.#input.destroy();
    scene.renderError.removeEventListener(this.#onRenderError);
    scene.morphComplete.removeEventListener(this.#onMorphComplete);
    scene.canvas.removeEventListener('webglcontextlost', this.#onContextLost);
    this.#clearSite();
    this.#widget.destroy();
  }

  #fixed(eMm: number, nMm: number, uMm: number): Cartesian3 {
    const frame = this.#frame;
    if (!frame) throw new Error('scene frame not set');
    return Matrix4.multiplyByPoint(
      frame.enuToFixed,
      new Cartesian3(eMm / 1000, nMm / 1000, uMm / 1000),
      new Cartesian3(),
    );
  }

  #position(p: PointMmDto): Cartesian3 {
    return this.#fixed(p.x_mm, p.y_mm, p.z_mm);
  }

  #ringPositions(dto: EntityDto): Cartesian3[] {
    const { x_mm, y_mm, z_mm } = dto.position;
    const out: Cartesian3[] = [];
    for (let i = 0; i < RING_SEGMENTS; i++) {
      const a = (2 * Math.PI * i) / RING_SEGMENTS;
      out.push(
        this.#fixed(x_mm + dto.sigma_mm * Math.cos(a), y_mm + dto.sigma_mm * Math.sin(a), z_mm),
      );
    }
    return out;
  }

  #addEntity(dto: EntityDto): EntityGraphics {
    const id: Pick = { kind: 'entity', id: dto.id };
    const position = this.#position(dto.position);
    const g: EntityGraphics = {
      dto,
      point: this.#entityPoints.add({
        id,
        position,
        pixelSize: 14,
        outlineWidth: 2,
        disableDepthTestDistance: Number.POSITIVE_INFINITY,
      }),
      label: this.#entityLabels.add({
        id,
        position,
        font: '13px system-ui, sans-serif',
        fillColor: Color.WHITE,
        outlineColor: Color.BLACK,
        outlineWidth: 3,
        style: LabelStyle.FILL_AND_OUTLINE,
        verticalOrigin: VerticalOrigin.BOTTOM,
        pixelOffset: new Cartesian2(0, -12),
        disableDepthTestDistance: Number.POSITIVE_INFINITY,
      }),
      ring: this.#rings.add({ id, positions: this.#ringPositions(dto), width: 2, loop: true }),
    };
    this.#styleEntity(g, dto);
    return g;
  }

  #updateEntity(g: EntityGraphics, dto: EntityDto): void {
    const moved =
      g.dto.position.x_mm !== dto.position.x_mm ||
      g.dto.position.y_mm !== dto.position.y_mm ||
      g.dto.position.z_mm !== dto.position.z_mm ||
      g.dto.sigma_mm !== dto.sigma_mm;
    if (moved) {
      const position = this.#position(dto.position);
      g.point.position = position;
      g.label.position = position;
      g.ring.positions = this.#ringPositions(dto);
    }
    g.dto = dto;
    this.#styleEntity(g, dto);
  }

  #styleEntity(g: EntityGraphics, dto: EntityDto): void {
    const alpha = FRESHNESS_ALPHA[dto.freshness];
    const base =
      dto.freshness === 'expired' ? Color.GRAY : Color.fromCssColorString(CLASS_COLOR[dto.class]);
    g.point.color = base.withAlpha(alpha);
    g.label.fillColor = Color.WHITE.withAlpha(Math.max(alpha, 0.6));
    const suffix = dto.freshness === 'active' ? '' : ` · ${dto.freshness}`;
    g.label.text = `${CLASS_LABEL[dto.class]} ${dto.id}${suffix}`;
    g.ring.material = Material.fromType('Color', { color: base.withAlpha(alpha * 0.7) });
  }

  #addMarker(dto: AnnotationDto): MarkerGraphics {
    const id: Pick = { kind: 'annotation', id: dto.id };
    const position = this.#position(dto.position);
    return {
      dto,
      pin: this.#markerPins.add({
        id,
        position,
        image: this.#pinImage,
        verticalOrigin: VerticalOrigin.BOTTOM,
        disableDepthTestDistance: Number.POSITIVE_INFINITY,
      }),
      label: this.#markerLabels.add({
        id,
        position,
        // Untrusted text, drawn as glyphs; never parsed as markup.
        text: dto.label,
        font: '13px system-ui, sans-serif',
        fillColor: Color.fromCssColorString(MARKER_COLOR),
        outlineColor: Color.BLACK,
        outlineWidth: 3,
        style: LabelStyle.FILL_AND_OUTLINE,
        verticalOrigin: VerticalOrigin.BOTTOM,
        pixelOffset: new Cartesian2(0, -44),
        disableDepthTestDistance: Number.POSITIVE_INFINITY,
      }),
    };
  }

  #updateMarker(g: MarkerGraphics, dto: AnnotationDto): void {
    const position = this.#position(dto.position);
    g.pin.position = position;
    g.label.position = position;
    g.label.text = dto.label;
    g.dto = dto;
  }

  #restyleSelection(): void {
    const sel = this.#selection;
    for (const [id, g] of this.#entities) {
      const selected = sel?.kind === 'entity' && sel.id === id;
      g.point.pixelSize = selected ? 20 : 14;
      g.point.outlineColor = selected ? SELECTED_OUTLINE : Color.BLACK;
      g.point.outlineWidth = selected ? 4 : 2;
      g.ring.width = selected ? 3 : 2;
    }
    for (const [id, g] of this.#markers) {
      const selected = sel?.kind === 'annotation' && sel.id === id;
      g.pin.scale = selected ? 1.35 : 1;
    }
  }

  #clearContent(): void {
    this.#entityPoints.removeAll();
    this.#entityLabels.removeAll();
    this.#rings.removeAll();
    this.#markerPins.removeAll();
    this.#markerLabels.removeAll();
    this.#preview = null;
    this.#entities.clear();
    this.#markers.clear();
  }

  #clearSite(): void {
    const site = this.#site;
    if (!site) return;
    this.#site = null;
    const scene = this.#widget.scene;
    scene.imageryLayers.remove(site.ground, true);
    // The primitive collection destroys what it removes, releasing GPU memory.
    scene.primitives.remove(site.lines);
    scene.primitives.remove(site.points);
    scene.primitives.remove(site.labels);
  }

  #lookAt(center: Cartesian3, range: number): void {
    this.#widget.camera.flyToBoundingSphere(new BoundingSphere(center, range / FIT_RANGE_RADII), {
      duration: 0,
      offset: new HeadingPitchRange(0, VIEW_PITCH, range),
    });
    this.#widget.scene.requestRender();
  }

  #captureView(): ViewOfInterest | null {
    const { camera, canvas } = this.#widget.scene;
    const middle = new Cartesian2(canvas.clientWidth / 2, canvas.clientHeight / 2);
    const center = camera.pickEllipsoid(middle);
    if (!center) return null;
    const range =
      this.#widget.scene.mode === SceneMode.SCENE2D
        ? camera.positionCartographic.height
        : Cartesian3.distance(camera.positionWC, center);
    return { center, range };
  }

  /** The ground-plane point under a window position, in scene metres. The
   * synthetic ground is the ellipsoid at the anchor's height (0 m). The
   * scene's z = 0 plane is tangent to it at the anchor and within about
   * 2 mm of it over this site (d²/2R at the corners), so the height is set
   * to exactly 0 and the placement is labelled as the ground plane. */
  #groundPoint(windowPosition: Cartesian2): { x_m: number; y_m: number } | null {
    const frame = this.#frame;
    const hit = this.#widget.camera.pickEllipsoid(windowPosition);
    if (!frame || !hit) return null;
    const enu = Matrix4.multiplyByPoint(frame.fixedToEnu, hit, new Cartesian3());
    return { x_m: enu.x, y_m: enu.y };
  }

  readonly #onClick = (event: ScreenSpaceEventHandler.PositionedEvent): void => {
    if (this.#tool === 'place') {
      this.#events.onPlace(this.#groundPoint(event.position));
      return;
    }
    const picked: unknown = this.#widget.scene.pick(event.position);
    this.#events.onPick(pickOf(picked));
  };

  /** For asynchronous callbacks that can outlive the widget. */
  readonly #requestRender = (): void => {
    if (!this.#widget.isDestroyed()) this.#widget.scene.requestRender();
  };

  readonly #onMorphComplete = (): void => {
    const view = this.#pendingView;
    this.#pendingView = null;
    if (view) this.#lookAt(view.center, view.range);
    this.#events.onModeChange(this.mode);
  };

  readonly #onRenderError = (_scene: unknown, error: unknown): void => {
    this.#events.onRendererProblem(`rendering stopped: ${String(error)}`);
  };

  readonly #onContextLost = (): void => {
    this.#events.onRendererProblem('the WebGL context was lost; reload to restore the map');
  };
}

/** The scene object a Cesium pick result refers to, if it is one of ours. */
function pickOf(picked: unknown): Pick | null {
  if (typeof picked !== 'object' || picked === null || !('id' in picked)) return null;
  const id: unknown = picked.id;
  if (
    typeof id === 'object' &&
    id !== null &&
    'kind' in id &&
    'id' in id &&
    (id.kind === 'entity' || id.kind === 'annotation') &&
    typeof id.id === 'string'
  ) {
    return { kind: id.kind, id: id.id };
  }
  return null;
}
