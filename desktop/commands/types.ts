import type { AppState } from "../core/state";

/** What only the shell can do: open things in the OS, raise the window. */
export interface Host {
  openUrl(url: string): Promise<void>;
  /** Open a file or folder with the default application. */
  openPath(path: string): Promise<void>;
}

/** A webview-side `Channel`: the UI passes `{ __channel: id }` and receives each `send` as a message. */
export interface Channel<T> {
  send(message: T): void;
}

export interface Ctx {
  host: Host;
  channel<T>(ref: unknown): Channel<T>;
}

export type Handler = (app: AppState, args: any, ctx: Ctx) => unknown;
