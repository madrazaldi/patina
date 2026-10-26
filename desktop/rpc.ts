// The wire between the webview and the Bun main process (Electrobun RPC).
// Shared by `desktop/main.ts` and the webview shim `web/src/lib/ipc.ts`
// (type-only there).

import type { RPCSchema } from "electrobun/rpc";

export type PatinaRPC = {
  /** What the Bun process handles. */
  bun: RPCSchema<{
    requests: {
      /** Run backend command `cmd`; rejects with the error message. */
      invoke: { params: { cmd: string; args?: Record<string, unknown> }; response: unknown };
    };
    messages: Record<never, never>;
  }>;
  /** What the webview handles: the messages Bun pushes to it. */
  webview: RPCSchema<{
    requests: Record<never, never>;
    messages: {
      /** A core event (`stage`, `session`, `now-playing`, ...). */
      event: { name: string; payload: unknown };
      /** A message on a `Channel` the webview passed to a command. */
      channel: { id: number; message: unknown };
    };
  }>;
};

/** What a webview `Channel` serializes to inside command arguments. */
export interface ChannelRef {
  __channel: number;
}

export const isChannelRef = (v: unknown): v is ChannelRef =>
  typeof v === "object" && v !== null && typeof (v as ChannelRef).__channel === "number";
