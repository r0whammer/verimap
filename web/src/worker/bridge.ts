// UI-thread side of the core worker: starts it, negotiates the protocol,
// and carries requests and their answers. Domain decisions stay in the Rust
// core; this module only moves messages and tracks whether the core is
// usable.
import { UI_BUILD } from '../build-info.ts';
import {
  PROTOCOL_VERSION,
  type CoreEvent,
  type ProtocolVersion,
  type SceneUpdateDto,
  type SessionDto,
  type SiteDto,
  type UiRequest,
} from '../generated/protocol.ts';

/** What the UI knows about the core. */
export type CoreStatus =
  | { readonly state: 'starting' }
  | {
      readonly state: 'ready';
      readonly protocol: ProtocolVersion;
      readonly coreBuild: string;
      readonly site: SiteDto;
      readonly session: SessionDto;
      /** The complete scene at the handshake. */
      readonly initialScene: SceneUpdateDto;
    }
  | {
      readonly state: 'incompatible';
      readonly coreProtocol: ProtocolVersion;
      readonly uiProtocol: ProtocolVersion;
    }
  | { readonly state: 'failed'; readonly reason: string };

/** The part of `Worker` the bridge uses; tests substitute a fake. */
export interface WorkerPort {
  postMessage(message: UiRequest): void;
  terminate(): void;
  onmessage: ((event: MessageEvent) => void) | null;
  onerror: ((event: ErrorEvent) => void) | null;
}

/** How long the core may take to load and answer `hello`. */
export const HANDSHAKE_TIMEOUT_MS = 15_000;

/** Events that answer a request (everything but the handshake answers). */
export type CoreAnswer = Exclude<CoreEvent, { type: 'ready' } | { type: 'incompatible' }>;

const EVENT_TYPES: readonly string[] = [
  'ready',
  'scene',
  'entity_detail',
  'state_hash',
  'incompatible',
  'error',
] satisfies readonly CoreEvent['type'][];

function isCoreEvent(data: unknown): data is CoreEvent {
  return (
    typeof data === 'object' &&
    data !== null &&
    'type' in data &&
    typeof data.type === 'string' &&
    EVENT_TYPES.includes(data.type)
  );
}

function startCoreWorker(): WorkerPort {
  return new Worker(new URL('./core.worker.ts', import.meta.url), {
    type: 'module',
    name: 'verimap-core',
  });
}

function unavailable(reason: string): CoreAnswer {
  return { type: 'error', code: 'internal', message: reason };
}

export class CoreBridge {
  #status: CoreStatus = { state: 'starting' };
  readonly #listeners = new Set<(status: CoreStatus) => void>();
  readonly #worker: WorkerPort;
  readonly #timer: ReturnType<typeof setTimeout>;
  /** Answers still expected, oldest first. The worker answers every
   * request exactly once and in order, so the next answer is always for
   * the oldest pending request. */
  readonly #pending: ((event: CoreAnswer) => void)[] = [];
  #helloAnswered = false;

  constructor(worker: WorkerPort = startCoreWorker()) {
    this.#worker = worker;
    worker.onmessage = (event) => {
      this.#receive(event.data);
    };
    worker.onerror = (event) => {
      this.#fail(`core worker error: ${event.message || 'unknown'}`);
    };
    this.#timer = setTimeout(() => {
      if (this.#status.state === 'starting') {
        this.#fail(`core did not answer within ${HANDSHAKE_TIMEOUT_MS / 1000} s`);
      }
    }, HANDSHAKE_TIMEOUT_MS);
    worker.postMessage({ type: 'hello', protocol: PROTOCOL_VERSION, ui_build: UI_BUILD });
  }

  get status(): CoreStatus {
    return this.#status;
  }

  /** Calls `listener` on every status change; returns an unsubscribe function. */
  subscribe(listener: (status: CoreStatus) => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Sends `request` and resolves with the core's answer. If the core is
   * not ready, or stops before answering, resolves with an `internal`
   * error instead; it never rejects. */
  request(request: Exclude<UiRequest, { type: 'hello' }>): Promise<CoreAnswer> {
    if (this.#status.state !== 'ready') {
      return Promise.resolve(unavailable(`core is ${this.#status.state}`));
    }
    return new Promise((resolve) => {
      this.#pending.push(resolve);
      this.#worker.postMessage(request);
    });
  }

  dispose(): void {
    clearTimeout(this.#timer);
    this.#listeners.clear();
    this.#worker.terminate();
    this.#flush('core stopped');
  }

  #receive(data: unknown): void {
    if (!isCoreEvent(data)) {
      this.#fail('core sent an unrecognized event');
      return;
    }
    if (!this.#helloAnswered) {
      this.#helloAnswered = true;
      this.#handshake(data);
      return;
    }
    const resolve = this.#pending.shift();
    if (!resolve || data.type === 'ready' || data.type === 'incompatible') {
      this.#fail(`core sent an unexpected ${data.type} event`);
      return;
    }
    resolve(data);
  }

  #handshake(data: CoreEvent): void {
    clearTimeout(this.#timer);
    switch (data.type) {
      case 'ready':
        this.#set({
          state: 'ready',
          protocol: data.protocol,
          coreBuild: data.core_build,
          site: data.site,
          session: data.session,
          initialScene: data.scene,
        });
        break;
      case 'incompatible':
        // Stop talking to a core this UI does not understand.
        this.#worker.terminate();
        this.#set({
          state: 'incompatible',
          coreProtocol: data.core_protocol,
          uiProtocol: data.ui_protocol,
        });
        break;
      case 'error':
        this.#fail(`${data.code}: ${data.message}`);
        break;
      default:
        this.#fail(`core answered hello with ${data.type}`);
    }
  }

  #fail(reason: string): void {
    clearTimeout(this.#timer);
    this.#worker.terminate();
    this.#set({ state: 'failed', reason });
    this.#flush(reason);
  }

  #flush(reason: string): void {
    for (const resolve of this.#pending.splice(0)) resolve(unavailable(reason));
  }

  #set(status: CoreStatus): void {
    this.#status = status;
    for (const listener of this.#listeners) listener(status);
  }
}
