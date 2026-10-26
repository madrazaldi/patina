// Webview RPC <-> backend commands. The shell hands this the two things only it
// can do: push a message to the webview and open things in the OS.

import { dispatch } from "../commands";
import type { Channel, Host } from "../commands/types";
import { type ChannelRef, isChannelRef } from "../rpc";
import { type CoreEvent, type EventSink, eventName, eventPayload } from "../core/state";
import type { AppState } from "../core/state";

export interface Webview {
  event(name: string, payload: unknown): void;
  channel(id: number, message: unknown): void;
}

/** Forwards core events to the webview under their names. */
export class WebviewSink implements EventSink {
  view: Webview | null = null;
  /** Called with every event, whether or not a webview is listening yet. */
  onEvent: ((event: CoreEvent) => void) | null = null;
  emit(event: CoreEvent): void {
    this.onEvent?.(event);
    this.view?.event(eventName(event), eventPayload(event));
  }
}

export function createBridge(app: AppState, view: () => Webview | null, host: Host) {
  const ctx = {
    host,
    channel<T>(ref: unknown): Channel<T> {
      if (!isChannelRef(ref)) throw new Error("Expected a channel.");
      const id = (ref as ChannelRef).__channel;
      return { send: (message: T) => view()?.channel(id, message) };
    },
  };
  return {
    /** Handle a webview `invoke`; throws an Error whose message the UI shows. */
    invoke: async ({ cmd, args }: { cmd: string; args?: Record<string, unknown> }): Promise<unknown> => {
      try {
        return await dispatch(app, cmd, args, ctx);
      } catch (e) {
        throw e instanceof Error ? e : new Error(String(e));
      }
    },
  };
}
