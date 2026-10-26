import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { UI_BUILD } from '../build-info.ts';
import { PROTOCOL_VERSION, type CoreEvent, type UiRequest } from '../generated/protocol.ts';
import { CoreBridge, HANDSHAKE_TIMEOUT_MS, type CoreStatus, type WorkerPort } from './bridge.ts';

class FakeWorker implements WorkerPort {
  readonly sent: UiRequest[] = [];
  terminated = false;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: ErrorEvent) => void) | null = null;

  postMessage(message: UiRequest): void {
    this.sent.push(message);
  }

  terminate(): void {
    this.terminated = true;
  }

  reply(data: unknown): void {
    this.onmessage?.({ data } as MessageEvent);
  }
}

const site = {
  site_id: 'demo-site-alpha',
  name: 'Synthetic Site Alpha',
  anchor: { longitude_deg: -98.35, latitude_deg: 38.75, ellipsoidal_height_m: 0 },
  bounds: {
    x: { min_mm: -100_000, max_mm: 100_000 },
    y: { min_mm: -100_000, max_mm: 100_000 },
    z: { min_mm: -20_000, max_mm: 80_000 },
  },
};

function start(): { worker: FakeWorker; bridge: CoreBridge; seen: CoreStatus[] } {
  const worker = new FakeWorker();
  const bridge = new CoreBridge(worker);
  const seen: CoreStatus[] = [];
  bridge.subscribe((s) => seen.push(s));
  return { worker, bridge, seen };
}

describe('CoreBridge', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('opens with hello carrying the generated protocol version', () => {
    const { worker, bridge } = start();
    expect(worker.sent).toEqual([
      { type: 'hello', protocol: PROTOCOL_VERSION, ui_build: UI_BUILD },
    ]);
    expect(bridge.status).toEqual({ state: 'starting' });
  });

  it('becomes ready on a ready event', () => {
    const { worker, bridge } = start();
    const ready: CoreEvent = { type: 'ready', protocol: PROTOCOL_VERSION, core_build: 'c1', site };
    worker.reply(ready);
    expect(bridge.status).toEqual({
      state: 'ready',
      protocol: PROTOCOL_VERSION,
      coreBuild: 'c1',
      site,
    });
    // A ready core is not timed out later.
    vi.advanceTimersByTime(HANDSHAKE_TIMEOUT_MS * 2);
    expect(bridge.status.state).toBe('ready');
    expect(worker.terminated).toBe(false);
  });

  it('stops using an incompatible core', () => {
    const { worker, bridge } = start();
    const core = { major: PROTOCOL_VERSION.major + 1, minor: 0 };
    worker.reply({ type: 'incompatible', core_protocol: core, ui_protocol: PROTOCOL_VERSION });
    expect(bridge.status).toEqual({
      state: 'incompatible',
      coreProtocol: core,
      uiProtocol: PROTOCOL_VERSION,
    });
    expect(worker.terminated).toBe(true);
  });

  it('fails on a core error, an unrecognized event, a worker error, or a timeout', () => {
    const cases: [string, (w: FakeWorker) => void][] = [
      [
        'internal: boom',
        (w) => {
          w.reply({ type: 'error', code: 'internal', message: 'boom' });
        },
      ],
      [
        'unrecognized',
        (w) => {
          w.reply({ type: 'surprise' });
        },
      ],
      [
        'unrecognized',
        (w) => {
          w.reply('ready');
        },
      ],
      ['worker error', (w) => w.onerror?.({ message: 'load failed' } as ErrorEvent)],
      ['did not answer', () => vi.advanceTimersByTime(HANDSHAKE_TIMEOUT_MS)],
    ];
    for (const [expected, act] of cases) {
      const { worker, bridge, seen } = start();
      act(worker);
      expect(bridge.status.state, expected).toBe('failed');
      expect(bridge.status.state === 'failed' && bridge.status.reason).toContain(expected);
      expect(worker.terminated).toBe(true);
      expect(seen).toHaveLength(1);
    }
  });

  it('dispose terminates the worker and silences listeners', () => {
    const { worker, bridge, seen } = start();
    bridge.dispose();
    expect(worker.terminated).toBe(true);
    vi.advanceTimersByTime(HANDSHAKE_TIMEOUT_MS);
    expect(seen).toEqual([]);
  });
});
