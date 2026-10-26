// The Cesium adapter: the only code that creates, updates, or disposes
// Cesium objects. React renders controls around it and calls these methods;
// it never owns render objects. Scene content arrives from the Rust core as
// DTOs; this module only places it for rendering.
import 'cesium/Build/Cesium/Widgets/CesiumWidget/CesiumWidget.css';
import {
  BoundingSphere,
  Cartesian2,
  Cartesian3,
  Cartographic,
  CesiumWidget,
  Color,
  HeadingPitchRange,
  ImageryLayer,
  LabelCollection,
  LabelStyle,
  Material,
  Math as CesiumMath,
  Matrix4,
  PointPrimitiveCollection,
  PolylineCollection,
  Rectangle,
  SceneMode,
  SingleTileImageryProvider,
  Transforms,
  VerticalOrigin,
  WebMercatorProjection,
} from 'cesium';
import type { SiteDto } from '../generated/protocol.ts';
import { GROUND_COLOR, siteGroundImage, solidColorImage } from './siteGround.ts';

export type ViewMode = '2d' | '3d';

export interface MapEvents {
  /** A 2D/3D switch finished; `mode` is the mode now shown. */
  onModeChange: (mode: ViewMode) => void;
  /** The renderer hit a problem. The core session is unaffected. */
  onRendererProblem: (message: string) => void;
}

const MORPH_SECONDS = 0.6;
const VIEW_PITCH = CesiumMath.toRadians(-40);
/** Camera range used to fit a bounding sphere, in sphere radii. */
const FIT_RANGE_RADII = 2.5;

interface SiteLayer {
  readonly ground: ImageryLayer;
  readonly lines: PolylineCollection;
  readonly points: PointPrimitiveCollection;
  readonly labels: LabelCollection;
  readonly sphere: BoundingSphere;
}

/** A view to restore after a 2D/3D switch: what was centred, from how far. */
interface ViewOfInterest {
  readonly center: Cartesian3;
  readonly range: number;
}

export class CesiumMap {
  readonly #widget: CesiumWidget;
  readonly #events: MapEvents;
  #site: SiteLayer | null = null;
  #pendingView: ViewOfInterest | null = null;

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
    scene.renderError.addEventListener(this.#onRenderError);
    scene.morphComplete.addEventListener(this.#onMorphComplete);
    scene.canvas.addEventListener('webglcontextlost', this.#onContextLost);
  }

  /** The mode shown, or the target mode while a switch is in progress. */
  get mode(): ViewMode {
    return this.#widget.scene.mode === SceneMode.SCENE2D ? '2d' : '3d';
  }

  get switching(): boolean {
    return this.#widget.scene.mode === SceneMode.MORPHING;
  }

  /** Replaces the drawn site: reference ground, scene-bounds box, anchor. */
  setSite(site: SiteDto): void {
    this.#clearSite();
    const scene = this.#widget.scene;
    const { anchor, bounds } = site;
    const { x, y, z } = bounds;
    const origin = Cartesian3.fromDegrees(
      anchor.longitude_deg,
      anchor.latitude_deg,
      anchor.ellipsoidal_height_m,
    );
    const enuToFixed = Transforms.eastNorthUpToFixedFrame(origin);
    const at = (eMm: number, nMm: number, uMm: number) =>
      Matrix4.multiplyByPoint(
        enuToFixed,
        new Cartesian3(eMm / 1000, nMm / 1000, uMm / 1000),
        new Cartesian3(),
      );

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
    scene.renderError.removeEventListener(this.#onRenderError);
    scene.morphComplete.removeEventListener(this.#onMorphComplete);
    scene.canvas.removeEventListener('webglcontextlost', this.#onContextLost);
    this.#clearSite();
    this.#widget.destroy();
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
