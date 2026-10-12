<script lang="ts">
  import { appInfo, type AppInfo, type Settings } from "./lib/api";
  import ImportView from "./lib/import/ImportView.svelte";
  import SettingsPanel from "./lib/import/SettingsPanel.svelte";
  import LibraryView from "./lib/library/LibraryView.svelte";

  type Tab = "library" | "import";

  let info = $state<AppInfo | null>(null);
  let error = $state<string | null>(null);
  let settings = $state<Settings | null>(null);
  let tab = $state<Tab>("library");

  const versionLabel = (i: AppInfo) => `v${i.version}`;

  $effect(() => {
    appInfo()
      .then((result) => (info = result))
      .catch((e: unknown) => (error = String(e)));
  });
</script>

<div class="app">
  <header>
    <h1>Picturesque</h1>
    {#if info}
      <nav aria-label="Main">
        <button
          class:active={tab === "library"}
          aria-current={tab === "library" ? "page" : undefined}
          onclick={() => (tab = "library")}
        >
          Library
        </button>
        <button
          class:active={tab === "import"}
          aria-current={tab === "import" ? "page" : undefined}
          onclick={() => (tab = "import")}
        >
          Import
        </button>
      </nav>
      <p
        class="version"
        data-testid="app-version"
        title={`${info.name} ${info.version}`}
      >
        {versionLabel(info)}
      </p>
    {:else if error}
      <p class="error" role="alert">Backend unavailable: {error}</p>
    {/if}
  </header>

  {#if info}
    <main class:full={tab === "library"}>
      <!-- Both views stay mounted so an import keeps running while browsing. -->
      <div class="view" hidden={tab !== "library"}>
        <LibraryView onimport={() => (tab = "import")} />
      </div>
      <div class="view import" hidden={tab !== "import"}>
        <aside>
          <SettingsPanel onchange={(s) => (settings = s)} />
        </aside>
        <div class="content">
          <ImportView hasLibrary={!!settings?.library_root} />
        </div>
      </div>
    </main>
  {/if}
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  header {
    display: flex;
    align-items: center;
    gap: 1.2rem;
    padding: 0.7rem 1.5rem;
    border-bottom: 1px solid var(--border);
  }

  header h1 {
    font-size: 1.25rem;
  }

  header p {
    margin: 0;
  }

  nav {
    display: flex;
    gap: 0.3rem;
  }

  nav button {
    border: none;
    background: none;
    padding: 0.35rem 0.8rem;
    border-radius: 6px;
    color: var(--muted);
    font-weight: 600;
  }

  nav button.active {
    background: var(--surface-2);
    color: var(--fg);
  }

  .version {
    margin-left: auto !important;
    color: var(--muted);
    font-size: 0.85rem;
  }

  .error {
    color: var(--danger);
  }

  main {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 1rem 1.5rem;
  }

  main.full {
    overflow: hidden;
  }

  .view {
    height: 100%;
  }

  .view.import {
    display: grid;
    grid-template-columns: minmax(18rem, 24rem) minmax(0, 1fr);
    gap: 1.2rem;
    align-items: start;
    height: auto;
  }

  .view[hidden] {
    display: none;
  }

  @media (max-width: 900px) {
    .view.import {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
