// Cross-process transactions and atomic, private file replacement.
// Port of crates/core/src/storage.rs.
//
// The locks are OS file locks on `<path>.lock` (Rust: fs2, i.e. flock(2) on Unix and
// LockFileEx on Windows), so a TUI process and the desktop app exclude each other.
// They are reached through `bun:ffi`, and everything stays synchronous like the Rust code.

import { closeSync, fsyncSync, mkdirSync, openSync, readFileSync, renameSync, rmSync, writeSync, existsSync } from "node:fs";
import { basename, dirname, extname, join } from "node:path";
import { randomBytes } from "node:crypto";

/** A held lock; dropping it in Rust is `release()` here (also safe to call twice). */
export interface Lock {
  release(): void;
}

// ---------------------------------------------------------------------------
// Native locking
// ---------------------------------------------------------------------------

interface Native {
  /** Try to take an exclusive lock on the open `fd`: `true` taken, `false` held elsewhere. */
  tryLock(fd: number): boolean;
  unlock(fd: number): void;
}

let native: Native | undefined;

function loadNative(): Native {
  if (native) return native;
  // Loaded lazily so importing this module has no side effects.
  // eslint-disable-next-line @typescript-eslint/no-require-imports
  const { dlopen, FFIType, ptr } = require("bun:ffi") as typeof import("bun:ffi");
  if (process.platform === "win32") {
    const k32 = dlopen("kernel32.dll", {
      LockFileEx: { args: [FFIType.ptr, FFIType.u32, FFIType.u32, FFIType.u32, FFIType.u32, FFIType.ptr], returns: FFIType.i32 },
      UnlockFileEx: { args: [FFIType.ptr, FFIType.u32, FFIType.u32, FFIType.u32, FFIType.ptr], returns: FFIType.i32 },
      GetLastError: { args: [], returns: FFIType.u32 },
    });
    const crt = dlopen("msvcrt.dll", { _get_osfhandle: { args: [FFIType.i32], returns: FFIType.ptr } });
    const LOCKFILE_FAIL_IMMEDIATELY = 0x1;
    const LOCKFILE_EXCLUSIVE_LOCK = 0x2;
    const ERROR_LOCK_VIOLATION = 33;
    // fs2 locks the whole possible range: u32::MAX bytes low and high.
    const overlapped = () => new Uint8Array(32); // OVERLAPPED, zeroed (offset 0)
    native = {
      tryLock(fd) {
        const ov = overlapped();
        const ok = k32.symbols.LockFileEx(crt.symbols._get_osfhandle(fd), LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY, 0, 0xffffffff, 0xffffffff, ptr(ov));
        if (ok !== 0) return true;
        const err = k32.symbols.GetLastError();
        // Windows reports contention as ERROR_LOCK_VIOLATION, not WouldBlock.
        if (err === ERROR_LOCK_VIOLATION) return false;
        throw new Error(`LockFileEx failed (error ${err})`);
      },
      unlock(fd) {
        const ov = overlapped();
        k32.symbols.UnlockFileEx(crt.symbols._get_osfhandle(fd), 0, 0xffffffff, 0xffffffff, ptr(ov));
      },
    };
    return native;
  }
  const lib = process.platform === "darwin" ? "libSystem.B.dylib" : "libc.so.6";
  const libc = dlopen(lib, { flock: { args: [FFIType.i32, FFIType.i32], returns: FFIType.i32 } });
  // Same values on Linux and macOS.
  const LOCK_EX = 2;
  const LOCK_NB = 4;
  const LOCK_UN = 8;
  native = {
    tryLock(fd) {
      // errno is not reachable through ffi. A non-blocking flock on an fd we just opened can only fail
      // with EWOULDBLOCK (held elsewhere) in practice, so any failure means contention.
      return libc.symbols.flock(fd, LOCK_EX | LOCK_NB) === 0;
    },
    unlock(fd) {
      libc.symbols.flock(fd, LOCK_UN);
    },
  };
  return native;
}

const sleepCell = new Int32Array(new SharedArrayBuffer(4));
/** Block the thread for `ms` (stays synchronous, unlike setTimeout). */
function sleepSync(ms: number): void {
  Atomics.wait(sleepCell, 0, 0, ms);
}

/** `path.with_extension("lock")`: replaces the extension, or appends one when there is none. */
export function lockPath(path: string): string {
  const name = basename(path);
  const ext = extname(name);
  const stem = ext ? name.slice(0, name.length - ext.length) : name;
  return join(dirname(path), `${stem}.lock`);
}

function openLockFile(path: string): number {
  mkdirSync(dirname(path), { recursive: true });
  // read+write, create, no truncate.
  return openSync(lockPath(path), "a+");
}

function handle(fd: number): Lock {
  let held = true;
  return {
    release() {
      if (!held) return;
      held = false;
      try {
        loadNative().unlock(fd);
      } finally {
        closeSync(fd);
      }
    },
  };
}

/** Take the exclusive lock of `path`, waiting for other processes (and handles) to release it. */
export function lock(path: string): Lock {
  const fd = openLockFile(path);
  try {
    const n = loadNative();
    let delay = 1;
    while (!n.tryLock(fd)) {
      sleepSync(delay);
      delay = Math.min(delay * 2, 20);
    }
  } catch (e) {
    closeSync(fd);
    throw new Error(`locking saved state: ${(e as Error).message}`);
  }
  return handle(fd);
}

/** Take the lock of `path` if nobody holds it; `null` otherwise. */
export function tryLock(path: string): Lock | null {
  const fd = openLockFile(path);
  try {
    if (loadNative().tryLock(fd)) return handle(fd);
  } catch (e) {
    closeSync(fd);
    throw e;
  }
  closeSync(fd);
  return null;
}

// ---------------------------------------------------------------------------
// Atomic writes and JSON transactions
// ---------------------------------------------------------------------------

/** The temporary file is unique and private from creation, including on first sign-in. */
export function write(path: string, bytes: Uint8Array): void {
  const parent = dirname(path);
  mkdirSync(parent, { recursive: true });
  const temp = join(parent, `.tmp${randomBytes(6).toString("hex")}`);
  // 0o600 from creation, exclusive: never reuses another writer's file.
  const fd = openSync(temp, "wx", 0o600);
  try {
    let off = 0;
    while (off < bytes.length) off += writeSync(fd, bytes, off, bytes.length - off);
    fsyncSync(fd);
  } catch (e) {
    closeSync(fd);
    rmSync(temp, { force: true });
    throw e;
  }
  closeSync(fd);
  try {
    renameSync(temp, path);
  } catch (e) {
    rmSync(temp, { force: true });
    throw new Error(`saving ${path}: ${(e as Error).message}`);
  }
  if (process.platform !== "win32") {
    const dir = openSync(parent, "r");
    try {
      fsyncSync(dir);
    } finally {
      closeSync(dir);
    }
  }
}

type Default<T> = T | (() => T);
const fresh = <T>(def: Default<T>): T => (typeof def === "function" ? (def as () => T)() : structuredClone(def));

/** The JSON file at `path`, or `def` (a value, cloned, or a factory) when it doesn't exist. */
export function readJson<T>(path: string, def: Default<T>): T {
  let bytes: Buffer;
  try {
    bytes = readFileSync(path);
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "ENOENT") return fresh(def);
    throw e;
  }
  try {
    return JSON.parse(bytes.toString("utf8")) as T;
  } catch (e) {
    throw new Error(`reading ${path}: ${(e as Error).message}`);
  }
}

/** Pretty JSON like `serde_json::to_vec_pretty` (2-space indent). */
function pretty(value: unknown): string {
  return JSON.stringify(value, null, 2);
}

/**
 * A read-modify-write transaction under the file's lock. Corrupt state is never overwritten,
 * and an update that changes nothing leaves the file alone.
 */
export function updateJson<T, R>(path: string, def: Default<T>, update: (value: T) => R): R {
  const held = lock(path);
  try {
    const value = readJson<T>(path, def);
    const before = pretty(value);
    const result = update(value);
    const after = pretty(value);
    // Polling workers often have nothing to claim or recover. Keep those transactions read-only.
    if (before !== after || !existsSync(path)) write(path, new TextEncoder().encode(after));
    return result;
  } finally {
    held.release();
  }
}

/** Like [[updateJson]] for values that can't be changed in place (numbers, strings): `update` returns the new value. */
export function replaceJson<T>(path: string, def: Default<T>, update: (value: T) => T): T {
  const held = lock(path);
  try {
    const value = readJson<T>(path, def);
    const next = update(value);
    if (pretty(value) !== pretty(next) || !existsSync(path)) write(path, new TextEncoder().encode(pretty(next)));
    return next;
  } finally {
    held.release();
  }
}
