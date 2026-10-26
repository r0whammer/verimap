// Dedicated worker hosting the Rust core (WebAssembly). It holds no logic of
// its own: each request goes to the core as JSON and the core's single JSON
// event is posted back, in order. If the core fails to load, or traps (which
// leaves the WASM instance unusable), every later request is answered with an
// `internal` error rather than left hanging.
import type { CoreEvent, UiRequest } from '../generated/protocol.ts';
import init, { Core } from '../wasm/verimap_core.js';

declare const self: DedicatedWorkerGlobalScope;

let failure: string | null = null;

const core: Promise<Core | null> = init()
  .then(() => new Core())
  .catch((e: unknown) => {
    failure = `core failed to load: ${String(e)}`;
    return null;
  });

function internalError(message: string): CoreEvent {
  return { type: 'error', code: 'internal', message };
}

self.onmessage = (event: MessageEvent<UiRequest>) => {
  void core.then((c) => {
    if (c === null || failure !== null) {
      self.postMessage(internalError(failure ?? 'core unavailable'));
      return;
    }
    try {
      self.postMessage(JSON.parse(c.handle(JSON.stringify(event.data))) as CoreEvent);
    } catch (e) {
      failure = `core trapped: ${String(e)}`;
      self.postMessage(internalError(failure));
    }
  });
};
