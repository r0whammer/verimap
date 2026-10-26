// UI-thread side of the core worker: starts it, negotiates the protocol, and
// reports what the UI may rely on. Domain decisions stay in the Rust core.
import { UI_BUILD } from '../build-info.ts';
import {
  PROTOCOL_VERSION,
  type CoreEvent,
  type ProtocolVersion,
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

const EVENT_TYPES: readonly string[] = [
  'ready',
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

export class CoreBridge {
  #status: CoreStatus = { state: 'starting' };
  readonly #listeners = new Set<(status: CoreStatus) => void>();
  readonly #worker: WorkerPort;
  readonly #timer: ReturnType<typeof setTimeout>;

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

  dispose(): void {
    clearTimeout(this.#timer);
    this.#listeners.clear();
    this.#worker.terminate();
  }

  #receive(data: unknown): void {
    if (!isCoreEvent(data)) {
      this.#fail('core sent an unrecognized event');
      return;
    }
    switch (data.type) {
      case 'ready':
        clearTimeout(this.#timer);
        this.#set({
          state: 'ready',
          protocol: data.protocol,
          coreBuild: data.core_build,
          site: data.site,
        });
        break;
      case 'incompatible':
        // Stop talking to a core this UI does not understand.
        clearTimeout(this.#timer);
        this.#worker.terminate();
        this.#set({
          state: 'incompatible',
          coreProtocol: data.core_protocol,
          uiProtocol: data.ui_protocol,
        });
        break;
      case 'error':
        // The only request so far is `hello`, so any error is fatal.
        this.#fail(`${data.code}: ${data.message}`);
        break;
    }
  }

  #fail(reason: string): void {
    clearTimeout(this.#timer);
    this.#worker.terminate();
    this.#set({ state: 'failed', reason });
  }

  #set(status: CoreStatus): void {
    this.#status = status;
    for (const listener of this.#listeners) listener(status);
  }
}
