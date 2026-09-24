// Async data for components. The loader runs in an effect, so any state it
// reads synchronously (arguments, `app.revision`) re-runs it when changed.
// Previous data stays visible while a refetch is in flight.

import { message } from "./state.svelte";

export class Resource<T> {
  data = $state<T | undefined>(undefined);
  error = $state<string | undefined>(undefined);
  loading = $state(true);
  #generation = 0;
  #nudge = $state(0);

  constructor(load: () => Promise<T>) {
    $effect(() => {
      this.#nudge;
      const generation = ++this.#generation;
      this.loading = true;
      load().then(
        (data) => {
          if (generation !== this.#generation) return;
          this.data = data;
          this.error = undefined;
          this.loading = false;
        },
        (e) => {
          if (generation !== this.#generation) return;
          this.error = message(e);
          this.loading = false;
        },
      );
    });
  }

  reload() {
    this.#nudge++;
  }
}

export const resource = <T>(load: () => Promise<T>) => new Resource(load);
