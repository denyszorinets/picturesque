<script lang="ts">
  import * as api from "../api";
  import type { ImportSource, Settings, TemplatePresetView } from "../api";
  import ModelList from "../ai/ModelList.svelte";
  import PathInput from "../components/PathInput.svelte";

  interface Props {
    /** Called when the wizard is finished or closed. */
    onclose: () => void;
    /** Called after a folder was opened to cull in place instead. */
    onopened?: (settings: Settings) => void;
  }

  let { onclose, onopened }: Props = $props();

  /** Instead of a library: cull a folder where it is (ADR-0013). */
  let inPlace = $state(false);
  let folderDraft = $state("");

  async function openInPlace(path: string) {
    if (!path.trim()) return;
    await run(async () => {
      const settings = await api.openFolder(path.trim());
      onopened?.(settings);
    });
  }

  async function chooseInPlace() {
    const picked = await api.pickFolders({
      multiple: false,
      title: "Folder to cull in place",
    });
    if (picked === null) inPlace = true;
    else if (picked[0]) await openInPlace(picked[0]);
  }

  const STEPS = ["Output folder", "Your photos", "AI models"] as const;
  let step = $state(0);

  let saved = $state<Settings | null>(null);
  let presets = $state<TemplatePresetView[]>([]);
  let libraryRoot = $state("");
  let template = $state("");
  let sources = $state<ImportSource[]>([]);
  let draft = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);

  const hasLibrary = $derived(!!saved?.library_root);
  const last = $derived(step === STEPS.length - 1);
  const canGoOn = $derived(
    saved !== null && (step !== 0 || libraryRoot.trim() !== ""),
  );

  $effect(() => {
    Promise.all([api.getSettings(), api.templatePresets()])
      .then(([settings, list]) => {
        saved = settings;
        presets = list;
        libraryRoot = settings.library_root ?? "";
        template = settings.path_template;
      })
      .catch((e: unknown) => (error = api.ApiError.from(e).message));
    loadSources();
  });

  function loadSources() {
    api
      .listImportSources()
      .then((list) => (sources = list))
      .catch((e: unknown) => (error = api.ApiError.from(e).message));
  }

  /** Runs a step's change; stays on the step and shows why if it fails. */
  async function run(change: () => Promise<void>): Promise<boolean> {
    busy = true;
    error = null;
    try {
      await change();
      return true;
    } catch (e) {
      error = api.ApiError.from(e).message;
      return false;
    } finally {
      busy = false;
    }
  }

  function saveLibrary(): Promise<boolean> {
    return run(async () => {
      if (!saved) return;
      const update: api.SettingsUpdate = {};
      const root = libraryRoot.trim();
      if (root !== (saved.library_root ?? "")) update.library_root = root;
      if (template !== saved.path_template) update.path_template = template;
      if (Object.keys(update).length > 0)
        saved = await api.updateSettings(update);
    });
  }

  async function addSources(paths: string[]) {
    const added = await run(async () => {
      for (const path of paths.map((p) => p.trim()).filter(Boolean))
        await api.addImportSource(path);
    });
    if (added) draft = "";
    loadSources();
  }

  async function next() {
    if (step === 0 && !(await saveLibrary())) return;
    if (last) onclose();
    else step += 1;
  }

  function skip() {
    error = null;
    if (last) onclose();
    else step += 1;
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
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
    aria-labelledby="setup-title"
    data-testid="setup-wizard"
  >
    <header>
      <div>
        <h2 id="setup-title">Welcome to Picturesque</h2>
        <p class="muted step">Step {step + 1} of {STEPS.length}</p>
      </div>
      <button class="close" aria-label="Close" onclick={onclose}>✕</button>
    </header>

    <ol class="progress" aria-label="Setup steps">
      {#each STEPS as label, i (label)}
        <li class:done={i < step} class:current={i === step}>{label}</li>
      {/each}
    </ol>

    {#if !saved}
      {#if !error}<p class="muted">Loading…</p>{/if}
    {:else if step === 0}
      <section aria-label="Output folder">
        <h3>Choose the output folder for your library</h3>
        <p class="muted">
          Picturesque <strong>copies</strong> your photos into this folder and organizes
          the copies here. Culling, sorting and exporting work on these copies. Choose
          an empty folder with enough free space.
        </p>
        <p class="safe" data-testid="setup-copy-only">
          Your originals are never moved, renamed, edited or deleted:
          Picturesque only reads them.
        </p>
        <PathInput
          label="Output folder"
          bind:value={libraryRoot}
          testid="setup-library-root"
          onsubmit={() => canGoOn && void next()}
        />
        <div class="in-place" data-testid="setup-in-place">
          {#if inPlace}
            <PathInput
              label="Folder to cull in place"
              bind:value={folderDraft}
              onsubmit={(p) => void openInPlace(p)}
              testid="setup-in-place-path"
            />
            <button
              class="small"
              disabled={busy || !folderDraft.trim()}
              onclick={() => void openInPlace(folderDraft)}>Open</button
            >
          {:else}
            <button
              class="link"
              disabled={busy}
              onclick={() => void chooseInPlace()}
              data-testid="setup-open-folder"
              >Or cull a folder where it is, without a library…</button
            >
            <span class="muted small-note"
              >Nothing is copied; ratings go to XMP sidecars next to the photos
              (for example a darktable folder or a memory card).</span
            >
          {/if}
        </div>
        <fieldset>
          <legend>Sort photos into</legend>
          {#each presets as preset (preset.id)}
            <label class="preset">
              <input
                type="radio"
                name="setup-preset"
                value={preset.template}
                bind:group={template}
              />
              <span class="preset-text">
                <span>{preset.label}</span>
                <code>{preset.template}</code>
              </span>
            </label>
          {/each}
        </fieldset>
      </section>
    {:else if step === 1}
      <section aria-label="Your photos">
        <h3>Where are your photos now?</h3>
        <p class="muted">
          Add the folders to import from, such as a memory card or an old photo
          folder, outside the output folder. Their photos are
          <strong>copied</strong> into the output folder, and new photos in them are
          copied automatically.
        </p>
        <p class="safe">
          These folders are only read: nothing in them is ever changed, moved or
          deleted.
        </p>
        {#if !hasLibrary}
          <p class="notice" data-testid="setup-needs-library">
            No output folder is set yet: photos wait until you choose one (go
            back, or later in Settings → Library folder).
          </p>
        {/if}
        {#if sources.length > 0}
          <ul class="sources" data-testid="setup-sources">
            {#each sources as source (source.id)}
              <li><code>{source.path}</code></li>
            {/each}
          </ul>
        {/if}
        <div class="add">
          <PathInput
            label="Folder to import from"
            bind:value={draft}
            multiple
            onpick={(paths) => void addSources(paths)}
            onsubmit={(path) => void addSources([path])}
            testid="setup-source"
          />
          <button
            onclick={() => void addSources([draft])}
            disabled={busy || draft.trim() === ""}
            data-testid="setup-add-source">Add</button
          >
        </div>
      </section>
    {:else}
      <section aria-label="AI models">
        <h3>Download AI models? (optional)</h3>
        <p class="muted">
          Aesthetic scores, similar photos, text search and closed-eye detection
          need AI models. They are downloaded once, then run on this computer
          offline. Downloads continue in the background; you can also get them
          later in Settings → AI models.
        </p>
        <ModelList />
      </section>
    {/if}

    {#if error}
      <p class="error-text" role="alert">{error}</p>
    {/if}

    <footer>
      {#if step > 0}
        <button onclick={() => (step -= 1)} disabled={busy}>Back</button>
      {/if}
      <span class="spacer"></span>
      {#if !last}
        <button onclick={skip} disabled={busy} data-testid="setup-skip"
          >Skip</button
        >
      {/if}
      <button
        class="primary"
        onclick={() => void next()}
        disabled={busy || !canGoOn}
        data-testid="setup-next">{last ? "Done" : "Next"}</button
      >
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
    display: grid;
    place-items: center;
    background: var(--scrim);
  }

  .dialog {
    width: min(38rem, calc(100vw - 2rem));
    max-height: calc(100vh - 2rem);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    background: var(--bg);
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
  }

  h2 {
    font-size: 1.2rem;
  }

  h3 {
    font-size: 1rem;
    margin-bottom: 0.3rem;
  }

  .step {
    margin: 0.15rem 0 0;
    font-size: 0.85rem;
  }

  .close {
    border: none;
    background: none;
    color: var(--muted);
  }

  .progress {
    display: flex;
    gap: 0.4rem;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 0.82rem;
  }

  .progress li {
    flex: 1;
    padding-top: 0.35rem;
    border-top: 3px solid var(--border);
    color: var(--muted);
  }

  .progress li.done {
    border-color: var(--accent);
  }

  .progress li.current {
    border-color: var(--accent);
    color: var(--fg);
    font-weight: 600;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  section p {
    margin: 0;
  }

  fieldset {
    border: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  legend {
    font-weight: 600;
    margin-bottom: 0.3rem;
  }

  .preset {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
  }

  .preset-text {
    display: flex;
    flex-direction: column;
  }

  .preset code {
    color: var(--muted);
    font-size: 0.8rem;
  }

  .sources {
    margin: 0;
    padding-left: 1.2rem;
  }

  .add {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  .add :global(.path-input) {
    flex: 1;
  }

  .safe {
    padding: 0.5rem 0.7rem;
    border-left: 3px solid var(--accent);
    border-radius: 4px;
    background: var(--surface-2);
  }

  .notice {
    padding: 0.5rem 0.7rem;
    border-radius: 6px;
    background: var(--surface-2);
  }

  footer {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  .spacer {
    flex: 1;
  }

  .in-place {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.3rem 0.6rem;
    margin: 0.8rem 0;
  }

  .small-note {
    font-size: 0.82rem;
  }
</style>
