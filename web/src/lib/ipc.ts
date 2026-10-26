// The webview's line to the backend: `invoke` runs a command in the Bun main
// process (see desktop/commands) and `listen` follows the events it pushes.
// It rides on Electrobun's browser RPC. When a test page installs
// `window.__PATINA_IPC__` (web/test/bridge.ts), that stands in for the backend.

import { Electroview } from "electrobun/view";
import type { PatinaRPC } from "../../../desktop/rpc";

export interface Event<T> {
  event: string;
  payload: T;
}

/** A test double for the backend. */
export interface IpcBridge {
  invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown>;
  listen(event: string, handler: (e: Event<unknown>) => void): () => void;
}

declare global {
  interface Window {
    __PATINA_IPC__?: IpcBridge;
    /** Set by the shell before the page loads. */
    __PATINA__?: { images?: string };
  }
}

type Handler = (e: Event<any>) => void;
const listeners = new Map<string, Set<Handler>>();
const channels = new Map<number, Channel<any>>();
let nextChannel = 0;

function emit(name: string, payload: unknown) {
  for (const handler of listeners.get(name) ?? []) handler({ event: name, payload });
}

let backend: ReturnType<typeof connect> | null = null;

function connect() {
  const rpc = Electroview.defineRPC<PatinaRPC>({
    // Commands such as complete_pin wait for the user; never time them out.
    maxRequestTime: Infinity,
    handlers: {
      requests: {},
      messages: {
        event: ({ name, payload }) => emit(name, payload),
        channel: ({ id, message }) => channels.get(id)?.onmessage?.(message),
      },
    },
  });
  new Electroview({ rpc });
  return rpc;
}

/** Messages the backend pushes during one call, like a Tauri channel. Pass it in the command's arguments. */
export class Channel<T> {
  readonly id = ++nextChannel;
  onmessage: ((message: T) => void) | undefined;
  constructor() {
    channels.set(this.id, this);
  }
  toJSON() {
    return { __channel: this.id };
  }
}

/** Run backend command `cmd`. Rejects with the error message (a string). */
export async function invoke<T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const test = window.__PATINA_IPC__;
  if (test) return (await test.invoke(cmd, args)) as T;
  backend ??= connect();
  const channelIds = Object.values(args ?? {}).filter((v): v is Channel<unknown> => v instanceof Channel);
  try {
    return (await backend.request.invoke({ cmd, args: args && JSON.parse(JSON.stringify(args)) })) as T;
  } catch (e) {
    throw e instanceof Error ? e.message : e;
  } finally {
    for (const c of channelIds) channels.delete(c.id);
  }
}

/** Follow backend event `event`. Resolves to a function that stops listening. */
export async function listen<T>(event: string, handler: (e: Event<T>) => void): Promise<() => void> {
  const test = window.__PATINA_IPC__;
  if (test) return test.listen(event, handler as (e: Event<unknown>) => void);
  backend ??= connect();
  let set = listeners.get(event);
  if (!set) listeners.set(event, (set = new Set()));
  set.add(handler);
  return () => void set.delete(handler);
}
