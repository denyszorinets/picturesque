<script lang="ts">
  import * as api from "../api";
  import type { ExportStatus, PhotoQuery } from "../api";
  import PathInput from "../components/PathInput.svelte";

  interface Props {
    /** Photos in the current view (filters applied). */
    view: PhotoQuery;
    viewCount: number;
    /** Ids of the selected photos. */
    selected: number[];
    picks: number;
    /** Photos auto-pick suggests keeping (without a flag). */
    autoPicked?: number;
    /** Photos not rejected. */
    notRejected?: number;
    /** Copying keepers out of a folder culled in place. */
    inFolder?: boolean;
    onclose: () => void;
  }

  let {
    view,
    viewCount,
    selected,
    picks,
    autoPicked = 0,
    notRejected = 0,
    inFolder = false,
    onclose,
  }: Props = $props();

  const STORAGE_KEY = "picturesque.export";
  const NAMING = [
    { template: "{name}.{ext}", label: "Original names" },
    { template: "{date}_{seq}.{ext}", label: "Date and number" },
    { template: "{year}/{date}/{name}.{ext}", label: "Folders by date" },
  ];
  const SIZES = [0, 4096, 2048, 1024];

  type Which = "picks" | "keepers" | "kept" | "view" | "selected";
  interface Saved {
    destination: string;
    format: "original" | "jpeg";
    naming: string;
    includeRaw: boolean;
    includeSidecars: boolean;
    maxEdge: number;
    quality: number;
  }

  function load(): Partial<Saved> {
    try {
      return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") as Saved;
    } catch {
      return {};
    }
  }
  const saved = load();

  // svelte-ignore state_referenced_locally
  let which = $state<Which>(
    selected.length > 0 ? "selected" : inFolder ? "keepers" : "picks",
  );
  let destination = $state(saved.destination ?? "");
  let format = $state<"original" | "jpeg">(saved.format ?? "original");
  let naming = $state(saved.naming ?? "{name}.{ext}");
  let includeRaw = $state(saved.includeRaw ?? true);
  let includeSidecars = $state(saved.includeSidecars ?? true);
  let maxEdge = $state(saved.maxEdge ?? 0);
  let quality = $state(saved.quality ?? 90);
  let status = $state<ExportStatus | null>(null);
  let error = $state<string | null>(null);

  const count = $derived(
    {
      picks,
      keepers: picks + autoPicked,
      kept: notRejected,
      view: viewCount,
      selected: selected.length,
    }[which],
  );
  const running = $derived(status?.state === "running");

  /** Progress events seen; a command's reply must not undo a newer one. */
  let events = 0;

  $effect(() => {
    return api.onAppEvent((event) => {
      if (event.type === "export_progress") {
        events += 1;
        status = event;
      }
    });
  });

  function query(): PhotoQuery | undefined {
    if (which === "picks") return undefined;
    // Flags and auto-picked photos match either.
    if (which === "keepers") return { flags: ["pick"], auto_picked: true };
    if (which === "kept") return { flags: ["pick", "unflagged"] };
    if (which === "view")
      return { ...view, cursor: undefined, limit: undefined };
    return { photo_ids: selected };
  }

  async function start() {
    error = null;
    try {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({
          destination,
          format,
          naming,
          includeRaw,
          includeSidecars,
          maxEdge,
          quality,
        } satisfies Saved),
      );
    } catch {
      // Remembering the options is a convenience only.
    }
    try {
      const seen = events;
      const started = await api.startExport({
        destination: destination.trim(),
        query: query(),
        format,
        naming,
        include_raw: includeRaw,
        include_sidecars: includeSidecars,
        max_edge: format === "jpeg" && maxEdge > 0 ? maxEdge : undefined,
        quality: format === "jpeg" ? quality : undefined,
      });
      // Progress may already have arrived (even the end of a small export).
      if (events === seen) status = started;
    } catch (e) {
      error = api.ApiError.from(e).message;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && !running) {
      e.stopPropagation();
      onclose();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" role="presentation">
  <div
    class="dialog card"
    role="dialog"
    aria-modal="true"
    aria-label="Export photos"
    data-testid="export-dialog"
  >
    <h2>{inFolder ? "Copy keepers" : "Export"}</h2>
    {#if inFolder && !status}
      <p class="muted hint">
        Copies the photos you keep to another folder, each copy verified, with
        XMP sidecars holding your decisions. The originals stay here.
      </p>
    {/if}
    {#if status && status.state !== "idle"}
      <div class="progress" data-testid="export-status">
        <progress max={status.total || 1} value={status.done}></progress>
        <p>
          {#if status.state === "running"}
            Exporting {status.done} of {status.total}…
          {:else if status.state === "cancelled"}
            Cancelled after {status.done} of {status.total} photos.
          {:else}
            Exported {status.done - status.failed.length}
            {status.done - status.failed.length === 1 ? "photo" : "photos"}
            ({status.files} files, all verified) to
            <code>{status.destination}</code>.
          {/if}
        </p>
        {#if status.failed.length > 0}
          <p class="error-text" data-testid="export-failed">
            {status.failed.length} could not be exported: {status.failed[0]!
              .error}
          </p>
        {/if}
        <div class="actions">
          {#if running}
            <button onclick={() => api.cancelExport()}>Cancel</button>
          {:else}
            <button class="primary" onclick={onclose} data-testid="export-done"
              >Done</button
            >
          {/if}
        </div>
      </div>
    {:else}
      <fieldset>
        <legend class="label">Photos</legend>
        <label class="radio"
          ><input
            type="radio"
            bind:group={which}
            value="keepers"
            data-testid="export-keepers"
          />
          Keepers: picks and auto-picked ({picks + autoPicked})</label
        >
        <label class="radio"
          ><input type="radio" bind:group={which} value="picks" /> Picks ({picks})</label
        >
        <label class="radio"
          ><input
            type="radio"
            bind:group={which}
            value="kept"
            data-testid="export-not-rejected"
          />
          Everything except rejects ({notRejected})</label
        >
        <label class="radio"
          ><input type="radio" bind:group={which} value="view" /> Photos in view
          ({viewCount})</label
        >
        <label class="radio"
          ><input
            type="radio"
            bind:group={which}
            value="selected"
            disabled={selected.length === 0}
          />
          Selected ({selected.length})</label
        >
      </fieldset>
      <div class="field">
        <span class="label">Folder</span>
        <PathInput
          label="Export folder"
          bind:value={destination}
          testid="export-destination"
        />
      </div>
      <fieldset>
        <legend class="label">Files</legend>
        <label class="radio"
          ><input type="radio" bind:group={format} value="original" /> Originals,
          unchanged</label
        >
        {#if format === "original"}
          <div class="sub">
            <label class="radio"
              ><input type="checkbox" bind:checked={includeRaw} /> RAW files too</label
            >
            <label class="radio"
              ><input type="checkbox" bind:checked={includeSidecars} /> XMP sidecars</label
            >
          </div>
        {/if}
        <label class="radio"
          ><input
            type="radio"
            bind:group={format}
            value="jpeg"
            data-testid="export-jpeg"
          /> JPEG</label
        >
        {#if format === "jpeg"}
          <div class="sub">
            <label class="radio"
              >Size
              <select bind:value={maxEdge} data-testid="export-size">
                {#each SIZES as size (size)}
                  <option value={size}
                    >{size === 0 ? "Full size" : `${size} px`}</option
                  >
                {/each}
              </select></label
            >
            <label class="radio"
              >Quality
              <input
                type="number"
                min="1"
                max="100"
                bind:value={quality}
              /></label
            >
          </div>
        {/if}
      </fieldset>
      <div class="field">
        <span class="label">Names</span>
        <select
          value={NAMING.some((n) => n.template === naming) ? naming : ""}
          onchange={(e) => {
            const v = (e.currentTarget as HTMLSelectElement).value;
            if (v) naming = v;
          }}
        >
          {#each NAMING as n (n.template)}
            <option value={n.template}>{n.label}</option>
          {/each}
          <option value="">Custom</option>
        </select>
        <input
          bind:value={naming}
          aria-label="Name template"
          data-testid="export-naming"
        />
      </div>
      {#if error}
        <p class="error-text" role="alert" data-testid="export-error">
          {error}
        </p>
      {/if}
      <div class="actions">
        <button onclick={onclose}>Cancel</button>
        <button
          class="primary"
          onclick={start}
          disabled={count === 0 || !destination.trim()}
          data-testid="export-start"
          >Export {count} {count === 1 ? "photo" : "photos"}</button
        >
      </div>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 20;
    display: grid;
    place-items: center;
    background: var(--scrim);
  }

  .dialog {
    width: min(34rem, calc(100vw - 2rem));
    max-height: calc(100vh - 2rem);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    background: var(--bg);
  }

  h2 {
    font-size: 1.1rem;
  }

  fieldset {
    border: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .label {
    font-weight: 600;
    font-size: 0.9rem;
    margin-bottom: 0.2rem;
  }

  .radio {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.9rem;
  }

  .sub {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem 1rem;
    margin-left: 1.5rem;
  }

  .sub input[type="number"] {
    width: 4.5rem;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  progress {
    width: 100%;
  }
</style>
