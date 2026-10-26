// A tiny unbounded multi-producer single-consumer channel, standing in for
// `tokio::sync::mpsc::unbounded_channel` (and `oneshot` via a one-item channel).

/** The sending and receiving ends of one queue. */
export class Channel<T> {
  private items: T[] = [];
  private waiter: ((value: { value: T } | null) => void) | null = null;
  private closedFlag = false;

  /** Queue a value. Returns false when the channel was closed (the value is dropped). */
  send(value: T): boolean {
    if (this.closedFlag) return false;
    const waiter = this.waiter;
    if (waiter) {
      this.waiter = null;
      waiter({ value });
    } else {
      this.items.push(value);
    }
    return true;
  }

  /** Wait for the next value; `null` once the channel is closed and drained. */
  recv(): Promise<T | null> {
    if (this.items.length > 0) return Promise.resolve(this.items.shift() as T);
    if (this.closedFlag) return Promise.resolve(null);
    return new Promise<T | null>((resolve) => {
      this.waiter = (box) => resolve(box ? box.value : null);
    });
  }

  /**
   * Like `recv`, but gives up after `ms` (`{ timeout: true }`), leaving the channel untouched so a
   * later `recv` still sees every value (a `tokio::select!` against a sleep).
   */
  recvTimeout(ms: number): Promise<{ timeout: true } | { timeout: false; value: T | null }> {
    if (this.items.length > 0) return Promise.resolve({ timeout: false, value: this.items.shift() as T });
    if (this.closedFlag) return Promise.resolve({ timeout: false, value: null });
    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        this.waiter = null;
        resolve({ timeout: true });
      }, Math.max(0, ms));
      this.waiter = (box) => {
        clearTimeout(timer);
        resolve({ timeout: false, value: box ? box.value : null });
      };
    });
  }

  /** The next value if one is queued right now. */
  tryRecv(): T | undefined {
    return this.items.shift();
  }

  /** Stop accepting values; a pending `recv` resolves with `null` once the queue is drained. */
  close(): void {
    this.closedFlag = true;
    const waiter = this.waiter;
    if (waiter) {
      this.waiter = null;
      waiter(null);
    }
  }

  get closed(): boolean {
    return this.closedFlag;
  }
}
