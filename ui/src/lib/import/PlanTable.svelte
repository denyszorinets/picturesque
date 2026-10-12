<script lang="ts">
  import type { ImportPlanView, PhotoStatus } from "../api";

  interface Props {
    plan: ImportPlanView;
  }

  let { plan }: Props = $props();

  type Filter = "all" | PhotoStatus;
  const PAGE = 200;
  const LABELS: Record<PhotoStatus, string> = {
    planned: "To import",
    imported: "Imported",
    duplicate: "Duplicate",
    failed: "Failed",
  };

  let filter = $state<Filter>("all");
  let shown = $state(PAGE);

  const counts = $derived.by(() => {
    const c: Record<PhotoStatus, number> = {
      planned: 0,
      imported: 0,
      duplicate: 0,
      failed: 0,
    };
    for (const p of plan.photos) c[p.status] += 1;
    return c;
  });
  const filters = $derived(
    (["all", "planned", "imported", "duplicate", "failed"] as Filter[]).filter(
      (f) => f === "all" || counts[f] > 0,
    ),
  );
  const visible = $derived(
    plan.photos.filter((p) => filter === "all" || p.status === filter),
  );

  function choose(f: Filter) {
    filter = f;
    shown = PAGE;
  }

  function name(path: string): string {
    return path.split(/[\\/]/).pop() ?? path;
  }
</script>

<div class="table-wrap">
  <div class="filters" role="tablist" aria-label="Filter photos">
    {#each filters as f (f)}
      <button
        role="tab"
        aria-selected={filter === f}
        class:active={filter === f}
        onclick={() => choose(f)}
      >
        {f === "all"
          ? `All ${plan.photos.length}`
          : `${LABELS[f]} ${counts[f]}`}
      </button>
    {/each}
  </div>

  <div class="scroll">
    <table data-testid="plan-table">
      <thead>
        <tr>
          <th>Status</th>
          <th>Photo</th>
          <th>Taken</th>
          <th>Destination</th>
        </tr>
      </thead>
      <tbody>
        {#each visible.slice(0, shown) as photo, i (i)}
          <tr>
            <td
              ><span class={`chip ${photo.status}`}>{LABELS[photo.status]}</span
              ></td
            >
            <td>
              {#each photo.files as file (file.source)}
                <div class="file" title={file.source}>{name(file.source)}</div>
              {/each}
            </td>
            <td class="when">
              {photo.captured_at?.replace("T", " ") ?? "—"}
              {#if photo.place}<div class="muted">{photo.place}</div>{/if}
            </td>
            <td>
              {#if photo.reason}
                <div class="reason">{photo.reason}</div>
              {:else}
                {#each photo.files as file (file.source)}
                  <code class="target">{file.target}</code>
                {/each}
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
  {#if visible.length > shown}
    <button class="more" onclick={() => (shown += PAGE)}
      >Show more ({visible.length - shown} left)</button
    >
  {/if}
</div>

<style>
  .table-wrap {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .filters button {
    padding: 0.25rem 0.7rem;
    font-size: 0.85rem;
    border-radius: 999px;
  }

  .filters button.active {
    background: var(--fg);
    color: var(--bg);
    border-color: var(--fg);
  }

  .scroll {
    overflow-x: auto;
    max-height: 26rem;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: 8px;
  }

  table {
    border-collapse: collapse;
    width: 100%;
    font-size: 0.85rem;
  }

  th {
    position: sticky;
    top: 0;
    background: var(--surface-2);
    text-align: left;
    font-weight: 600;
    padding: 0.45rem 0.6rem;
  }

  td {
    padding: 0.45rem 0.6rem;
    border-top: 1px solid var(--border);
    vertical-align: top;
  }

  .file,
  .target {
    display: block;
    overflow-wrap: anywhere;
  }

  .when {
    white-space: nowrap;
  }

  .reason {
    color: var(--muted);
  }

  .chip {
    display: inline-block;
    padding: 0.1rem 0.5rem;
    border-radius: 999px;
    font-size: 0.75rem;
    font-weight: 600;
    white-space: nowrap;
    color: #fff;
  }

  .chip.planned {
    background: var(--info);
  }

  .chip.imported {
    background: var(--accent);
  }

  .chip.duplicate {
    background: var(--muted);
  }

  .chip.failed {
    background: var(--danger);
  }

  .more {
    align-self: flex-start;
  }
</style>
