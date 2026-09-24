<script lang="ts">
  // Data sent per time bucket as stacked columns: remote (accent) on the
  // baseline, local network (neutral) above it. Hover a column for its values;
  // the same numbers are available as a table.
  import { bytes } from "../lib/format";
  import type { ServerStats } from "../lib/types";

  let {
    buckets,
    since,
    step,
  }: {
    buckets: ServerStats["bandwidth"];
    /** Start of the period, unix seconds. */
    since: number;
    /** Bucket length in seconds. */
    step: number;
  } = $props();

  const H = 168; // plot height
  const TOP = 8; // room for the top tick label
  const AXIS = 24; // x-axis label band
  const LEFT = 60; // y-axis label gutter
  const RIGHT = 12;
  const GAP = 2; // surface gap between stacked segments
  const UNITS = ["B", "KB", "MB", "GB", "TB"];

  let width = $state(0);
  let hover = $state<number | null>(null);
  let table = $state(false);

  const hourly = $derived(step < 86400);
  const hourFmt = new Intl.DateTimeFormat(undefined, { hour: "numeric" });
  const dayFmt = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
  const fullFmt = $derived(
    new Intl.DateTimeFormat(undefined, hourly ? { weekday: "short", hour: "numeric", minute: "2-digit" } : { weekday: "short", month: "short", day: "numeric" }),
  );
  const at = (ts: number) => new Date(ts * 1000);

  /** Buckets on a continuous axis aligned to the server's own boundaries, so quiet periods show as gaps. */
  let slots = $derived.by(() => {
    const end = Math.max(Date.now() / 1000, buckets.at(-1)?.at ?? 0);
    const base = buckets[0]?.at ?? since;
    const start = base - Math.max(0, Math.floor((base - since) / step)) * step;
    const n = Math.max(1, Math.floor((end - start) / step) + 1);
    const out = Array.from({ length: n }, (_, i) => ({ at: start + i * step, lan: 0, wan: 0 }));
    for (const b of buckets) {
      const i = Math.floor((b.at - start) / step);
      if (i < 0 || i >= n) continue;
      out[i].lan += b.lan;
      out[i].wan += b.wan;
    }
    return out;
  });

  /** Y axis in the largest binary unit that fits, with 1/2/2.5/5 steps. */
  let scale = $derived.by(() => {
    const max = Math.max(1, ...slots.map((s) => s.lan + s.wan));
    let k = 0;
    while (k < UNITS.length - 1 && max / 1024 ** (k + 1) >= 1) k++;
    const unit = 1024 ** k;
    const raw = max / unit / 3;
    const mag = 10 ** Math.floor(Math.log10(raw));
    const inc = [1, 2, 2.5, 5, 10].map((m) => m * mag).find((s) => s >= raw) ?? 10 * mag;
    const count = Math.max(1, Math.ceil(max / unit / inc - 1e-9));
    const ticks = Array.from({ length: count + 1 }, (_, i) => {
      const v = +(i * inc).toFixed(3);
      return { value: v * unit, label: v === 0 ? "0" : `${v} ${UNITS[k]}` };
    });
    return { top: count * inc * unit, ticks };
  });

  let plotW = $derived(Math.max(0, width - LEFT - RIGHT));
  let band = $derived(plotW / slots.length);
  let barW = $derived(Math.min(24, Math.max(2, band * 0.62)));
  const y = (v: number) => TOP + H - (v / scale.top) * H;

  /** Label the latest bucket and every 6th hour / 7th day back from it. */
  let every = $derived(hourly ? 6 : 7);
  const tickLabel = (ts: number) => (hourly ? hourFmt : dayFmt).format(at(ts));

  /** A column rising from the baseline, rounded only at the data end. */
  function column(x: number, top: number, w: number, h: number, round: boolean) {
    const r = round ? Math.min(4, w / 2, h) : 0;
    return `M${x},${top + h}V${top + r}A${r},${r} 0 0 1 ${x + r},${top}H${x + w - r}A${r},${r} 0 0 1 ${x + w},${top + r}V${top + h}Z`;
  }

  let columns = $derived(
    slots.map((s, i) => {
      const x = LEFT + i * band + (band - barW) / 2;
      const base = TOP + H;
      const hw = s.wan ? Math.max(1, (s.wan / scale.top) * H) : 0;
      const hl = s.lan ? Math.max(1, (s.lan / scale.top) * H) : 0;
      const wan = hw ? column(x, base - hw, barW, hw, !hl) : null;
      const lanH = hl ? Math.max(1, hl - (hw ? GAP : 0)) : 0;
      const lan = hl ? column(x, base - hw - hl, barW, lanH, true) : null;
      return { wan, lan };
    }),
  );

  function move(e: PointerEvent) {
    const rect = (e.currentTarget as SVGElement).getBoundingClientRect();
    const i = Math.floor((e.clientX - rect.left - LEFT) / band);
    hover = i >= 0 && i < slots.length ? i : null;
  }

  let totals = $derived(slots.reduce((t, s) => ({ lan: t.lan + s.lan, wan: t.wan + s.wan }), { lan: 0, wan: 0 }));
  let tip = $derived(hover == null || hover >= slots.length ? null : { slot: slots[hover], x: LEFT + (hover + 0.5) * band, right: hover > slots.length / 2 });
</script>

<div class="legend">
  <span><i class="key wan"></i>Remote <b class="num">{bytes(totals.wan)}</b></span>
  <span><i class="key lan"></i>Local network <b class="num">{bytes(totals.lan)}</b></span>
  <button class="link view" onclick={() => (table = !table)}>{table ? "Show chart" : "Show as table"}</button>
</div>

{#if table}
  <div class="table">
    <table class="num">
      <thead><tr><th>{hourly ? "Hour" : "Day"}</th><th>Remote</th><th>Local network</th><th>Total</th></tr></thead>
      <tbody>
        {#each slots.filter((s) => s.lan + s.wan > 0).reverse() as s (s.at)}
          <tr><td>{fullFmt.format(at(s.at))}</td><td>{bytes(s.wan)}</td><td>{bytes(s.lan)}</td><td>{bytes(s.lan + s.wan)}</td></tr>
        {/each}
      </tbody>
    </table>
  </div>
{:else}
  <div class="chart" bind:clientWidth={width}>
    {#if width}
      <svg {width} height={TOP + H + AXIS} role="img" aria-label="Data sent over time" onpointermove={move} onpointerleave={() => (hover = null)}>
        {#each scale.ticks as t (t.value)}
          <line class="grid" x1={LEFT} x2={width - RIGHT} y1={y(t.value)} y2={y(t.value)} />
          <text class="tick" x={LEFT - 10} y={y(t.value)} text-anchor="end" dominant-baseline="middle">{t.label}</text>
        {/each}
        {#each columns as c, i (slots[i].at)}
          <g class={{ dim: hover != null && hover !== i }}>
            {#if c.wan}<path class="wan" d={c.wan} />{/if}
            {#if c.lan}<path class="lan" d={c.lan} />{/if}
          </g>
          {#if (slots.length - 1 - i) % every === 0}
            <text class="tick" x={LEFT + (i + 0.5) * band} y={TOP + H + 16} text-anchor="middle">{tickLabel(slots[i].at)}</text>
          {/if}
        {/each}
      </svg>
      {#if tip}
        <div class={["tip", { right: tip.right }]} style:left="{tip.x}px">
          <div class="when">{fullFmt.format(at(tip.slot.at))}</div>
          {#if tip.slot.lan + tip.slot.wan === 0}
            <div class="faint">Nothing streamed</div>
          {:else}
            <div><i class="line wan"></i><b class="num">{bytes(tip.slot.wan)}</b> remote</div>
            <div><i class="line lan"></i><b class="num">{bytes(tip.slot.lan)}</b> local network</div>
          {/if}
        </div>
      {/if}
    {/if}
  </div>
{/if}

<style>
  .legend {
    display: flex;
    align-items: baseline;
    gap: 20px;
    margin-bottom: 14px;
    font-size: 12px;
    color: var(--text-2);
  }
  .legend b {
    margin-left: 4px;
    font-weight: 500;
    color: var(--text);
  }
  .view {
    margin-left: auto;
    font-size: 12px;
  }
  .key {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 7px;
    border-radius: 2px;
  }
  .key.wan,
  .line.wan {
    background: var(--accent);
  }
  .key.lan,
  .line.lan {
    background: var(--text-3);
  }
  .chart {
    position: relative;
  }
  svg {
    display: block;
    overflow: visible;
  }
  .grid {
    stroke: var(--line);
    stroke-width: 1;
    shape-rendering: crispEdges;
  }
  .tick {
    fill: var(--text-3);
    font-size: 10.5px;
    font-variant-numeric: tabular-nums;
  }
  path.wan {
    fill: var(--accent);
  }
  path.lan {
    fill: var(--text-3);
  }
  g {
    transition: opacity 0.12s var(--ease);
  }
  g.dim {
    opacity: 0.45;
  }
  .tip {
    position: absolute;
    top: 0;
    transform: translateX(14px);
    min-width: 150px;
    padding: 8px 10px;
    border: 1px solid var(--line-2);
    border-radius: var(--r);
    background: #181818;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    font-size: 12px;
    color: var(--text-2);
    white-space: nowrap;
    pointer-events: none;
  }
  .tip.right {
    transform: translateX(calc(-100% - 14px));
  }
  .tip .when {
    margin-bottom: 4px;
    color: var(--text-3);
  }
  .tip b {
    font-weight: 500;
    color: var(--text);
  }
  .line {
    display: inline-block;
    width: 10px;
    height: 2px;
    margin-right: 7px;
    vertical-align: middle;
    border-radius: 1px;
  }
  .table {
    max-height: 260px;
    overflow: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  th {
    position: sticky;
    top: 0;
    background: var(--bg);
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  th,
  td {
    padding: 6px 0;
    border-bottom: 1px solid var(--line);
    text-align: right;
  }
  th:first-child,
  td:first-child {
    text-align: left;
  }
  td {
    color: var(--text-2);
  }
</style>
