<script lang="ts">
  import * as api from "../api";
  import type { Settings } from "../api";
  import PathInput from "../components/PathInput.svelte";
  import { folderName, sessionLabel } from "./session";

  interface Props {
    settings: Settings;
    /** Called with the new session's settings after switching. */
    onswitch: (settings: Settings) => void;
    /** Whether the menu is open. */
    open?: boolean;
  }

  let { settings, onswitch, open = $bindable(false) }: Props = $props();

  /** A switch waiting for an answer: auto-pick labels are not written. */
  let confirming = $state<{
    labels: number;
    change: () => Promise<Settings>;
  } | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  /** The path field, where no folder dialog is available (a browser). */
  let typing = $state(false);
  let draft = $state("");
  let root: HTMLElement | undefined = $state();

  const recent = $derived(
    settings.recent_folders.filter((f) => f.path !== settings.open_folder),
  );

  /**
   * Leaving a folder whose auto-picked photos lack their label: ask first
   * (labels are written only when a folder is finished).
   */
  async function leave(change: () => Promise<Settings>) {
    if (settings.open_folder && settings.labels.auto_pick) {
      try {
        const status = await api.finishStatus();
        if (status.labels_to_write > 0) {
          confirming = { labels: status.labels_to_write, change };
          return;
        }
      } catch {
        // Unknown: switch anyway; decisions are in their sidecars.
      }
    }
    await run(change);
  }

  async function run(change: () => Promise<Settings>, finishFirst = false) {
    busy = true;
    error = null;
    try {
      if (finishFirst) await api.finishFolder();
      const next = await change();
      confirming = null;
      open = false;
      typing = false;
      draft = "";
      onswitch(next);
    } catch (e) {
      error = api.ApiError.from(e).message;
    } finally {
      busy = false;
    }
  }

  async function chooseFolder() {
    const picked = await api.pickFolders({
      multiple: false,
      title: "Folder to cull in place",
    });
    if (picked === null) {
      typing = true;
      return;
    }
    const [path] = picked;
    if (path) await leave(() => api.openFolder(path));
  }

  function onwindowclick(e: MouseEvent) {
    // The path at dispatch: a clicked item may be gone from the page by now.
    if (open && root && !e.composedPath().includes(root)) open = false;
  }
</script>

<svelte:window onclick={onwindowclick} />

<div class="switcher" bind:this={root}>
  <button
    class="current"
    class:folder={!!settings.open_folder}
    onclick={() => (open = !open)}
    aria-expanded={open}
    aria-haspopup="menu"
    aria-label={`Culling: ${sessionLabel(settings)}. Open another folder or the library`}
    title={settings.open_folder
      ? `Culling ${settings.open_folder} in place`
      : "Culling the library"}
    data-testid="session-switcher"
  >
    <svg
      class="icon"
      viewBox="0 0 16 16"
      width="14"
      height="14"
      aria-hidden="true"
    >
      {#if settings.open_folder}
        <path d="M1.5 3.5h4l1.5 1.5h7.5v8h-13z" />
      {:else}
        <rect x="1.5" y="2.5" width="13" height="11" rx="1.5" />
        <path d="M1.5 6.5h13M5.5 6.5v7" />
      {/if}
    </svg>
    <span class="name">{sessionLabel(settings)}</span>
    <span aria-hidden="true" class="caret">▾</span>
  </button>
  {#if open}
    <div class="menu" role="menu" data-testid="session-menu">
      <button
        role="menuitemradio"
        aria-checked={!settings.open_folder}
        disabled={busy || !settings.open_folder}
        onclick={() => void leave(api.openLibrary)}
        data-testid="open-library"
      >
        <span class="check">{settings.open_folder ? "" : "✓"}</span>
        <span>
          Library
          <span class="muted small">Photos copied into your library</span>
        </span>
      </button>
      {#if settings.open_folder}
        <div
          class="item current-folder"
          role="menuitemradio"
          aria-checked="true"
        >
          <span class="check">✓</span>
          <span>
            {folderName(settings.open_folder)}
            <span class="muted small path">{settings.open_folder}</span>
          </span>
        </div>
      {/if}
      {#if recent.length > 0}
        <p class="group muted">Recent folders</p>
        {#each recent as folder (folder.path)}
          <button
            role="menuitemradio"
            aria-checked="false"
            disabled={busy}
            title={folder.path}
            onclick={() => void leave(() => api.openFolder(folder.path))}
            data-testid="recent-folder"
          >
            <span class="check"></span>
            <span>
              {folderName(folder.path)}
              <span class="muted small path">{folder.path}</span>
            </span>
          </button>
        {/each}
      {/if}
      <hr />
      {#if typing}
        <div class="typing">
          <PathInput
            label="Folder to cull in place"
            bind:value={draft}
            onsubmit={(path) =>
              path.trim() && void leave(() => api.openFolder(path.trim()))}
            testid="open-folder-path"
          />
          <button
            class="primary small"
            disabled={busy || !draft.trim()}
            onclick={() => void leave(() => api.openFolder(draft.trim()))}
            data-testid="open-folder-go">Open</button
          >
        </div>
      {:else}
        <button
          role="menuitem"
          disabled={busy}
          onclick={() => void chooseFolder()}
          data-testid="open-folder"
        >
          <span class="check">＋</span>
          <span>
            Open folder…
            <span class="muted small"
              >Cull photos where they are; ratings go to XMP sidecars next to
              them</span
            >
          </span>
        </button>
      {/if}
      {#if confirming}
        {@const pending = confirming}
        <div class="confirm" role="alert" data-testid="switch-confirm">
          <p>
            {pending.labels}
            {pending.labels === 1 ? "photo" : "photos"} in this folder
            {pending.labels === 1 ? "has" : "have"} not got
            {pending.labels === 1 ? "its" : "their"} auto-pick label yet.
          </p>
          <div class="confirm-actions">
            <button
              class="primary small"
              disabled={busy}
              onclick={() => void run(pending.change, true)}
              data-testid="switch-finish">Write labels, then switch</button
            >
            <button
              class="small"
              disabled={busy}
              onclick={() => void run(pending.change)}
              data-testid="switch-anyway">Switch without</button
            >
            <button
              class="small"
              disabled={busy}
              onclick={() => (confirming = null)}>Cancel</button
            >
          </div>
        </div>
      {/if}
      {#if busy}<p class="muted small status">Opening…</p>{/if}
      {#if error}<p class="error-text small status" role="alert">
          {error}
        </p>{/if}
    </div>
  {/if}
</div>

<style>
  .switcher {
    position: relative;
  }

  .current {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    max-width: 16rem;
    padding: 0.3rem 0.7rem;
    border-radius: 999px;
    background: var(--surface-2);
    border-color: transparent;
    font-size: 0.9rem;
  }

  .current.folder {
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .icon {
    flex-shrink: 0;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linejoin: round;
  }

  .caret {
    color: var(--muted);
    font-size: 0.75rem;
  }

  .menu {
    position: absolute;
    left: 0;
    top: calc(100% + 0.4rem);
    z-index: 30;
    width: 22rem;
    max-width: calc(100vw - 2rem);
    padding: 0.4rem;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    box-shadow: 0 6px 24px rgb(0 0 0 / 18%);
  }

  .menu > button,
  .item {
    display: flex;
    gap: 0.5rem;
    width: 100%;
    padding: 0.45rem 0.5rem;
    border: none;
    border-radius: 6px;
    background: none;
    text-align: left;
    font-size: 0.9rem;
  }

  .menu > button:hover:not(:disabled) {
    background: var(--surface-2);
  }

  .menu > button:disabled {
    cursor: default;
  }

  .check {
    width: 1rem;
    flex-shrink: 0;
    color: var(--accent);
  }

  .small {
    display: block;
    font-size: 0.78rem;
  }

  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 18rem;
  }

  .group {
    margin: 0.5rem 0.5rem 0.2rem;
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  hr {
    border: none;
    border-top: 1px solid var(--border);
    margin: 0.35rem 0;
  }

  .typing {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    padding: 0.3rem;
  }

  .status {
    margin: 0.3rem 0.5rem;
  }

  .confirm {
    margin: 0.4rem;
    padding: 0.6rem;
    border-radius: 6px;
    background: color-mix(in srgb, var(--warn) 14%, transparent);
    font-size: 0.85rem;
  }

  .confirm p {
    margin: 0 0 0.5rem;
  }

  .confirm-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }
</style>
