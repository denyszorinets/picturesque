<script lang="ts">
  import * as api from "./lib/api";
  import type { AppInfo, Settings } from "./lib/api";
  import ActivityView from "./lib/activity/ActivityView.svelte";
  import PipelineIndicator from "./lib/activity/PipelineIndicator.svelte";
  import { Pipeline } from "./lib/activity/pipeline.svelte";
  import LibraryView from "./lib/library/LibraryView.svelte";
  import SettingsPanel from "./lib/settings/SettingsPanel.svelte";
  import SetupWizard from "./lib/setup/SetupWizard.svelte";
  import StatsView from "./lib/stats/StatsView.svelte";

  type Tab = "library" | "activity" | "statistics" | "settings";
  const TABS: { id: Tab; label: string }[] = [
    { id: "library", label: "Library" },
    { id: "activity", label: "Activity" },
    { id: "statistics", label: "Statistics" },
    { id: "settings", label: "Settings" },
  ];

  let info = $state<AppInfo | null>(null);
  let error = $state<string | null>(null);
  let settings = $state<Settings | null>(null);
  let tab = $state<Tab>("library");
  /** The setup wizard: opened on first launch, or from Settings. */
  let setup = $state(false);
  /** Bumped after the wizard so Settings shows what it saved. */
  let settingsRevision = $state(0);
  const pipeline = new Pipeline(api);

  const versionLabel = (i: AppInfo) => `v${i.version}`;

  $effect(() => {
    api
      .appInfo()
      .then((result) => (info = result))
      .catch((e: unknown) => (error = String(e)));
    api
      .getSettings()
      .then((s) => {
        settings = s;
        // First launch: nothing set up yet.
        if (!s.library_root) setup = true;
      })
      .catch(() => {});
  });

  async function closeSetup() {
    setup = false;
    tab = "library";
    settingsRevision += 1;
    void pipeline.loadStatus();
    try {
      settings = await api.getSettings();
    } catch {
      // Settings reloads itself.
    }
  }

  // The header indicator follows the pipeline everywhere.
  $effect(() => {
    void pipeline.loadStatus();
    const stop = api.onAppEvent((event) => pipeline.handle(event));
    // Events may be missed (for example while reconnecting).
    const timer = setInterval(() => {
      if (pipeline.busy) void pipeline.loadStatus();
    }, 3000);
    return () => {
      stop();
      clearInterval(timer);
    };
  });
</script>

<div class="app">
  <header>
    <h1>Picturesque</h1>
    {#if info}
      <nav aria-label="Main">
        {#each TABS as t (t.id)}
          <button
            class:active={tab === t.id}
            aria-current={tab === t.id ? "page" : undefined}
            onclick={() => (tab = t.id)}
          >
            {t.label}
          </button>
        {/each}
      </nav>
      <div class="status">
        <PipelineIndicator {pipeline} onclick={() => (tab = "activity")} />
        <p
          class="version"
          data-testid="app-version"
          title={`${info.name} ${info.version}`}
        >
          {versionLabel(info)}
        </p>
      </div>
    {:else if error}
      <p class="error" role="alert">Backend unavailable: {error}</p>
    {/if}
  </header>

  {#if info}
    <main class:full={tab === "library"}>
      <!-- Views stay mounted so they keep their state while switching. -->
      <div class="view" hidden={tab !== "library"}>
        <LibraryView
          onimport={() => (tab = "activity")}
          onsettings={() => (tab = "settings")}
          importing={pipeline.busy}
        />
      </div>
      <div class="view page" hidden={tab !== "activity"}>
        <ActivityView
          {pipeline}
          visible={tab === "activity"}
          hasLibrary={settings === null || !!settings.library_root}
          onsettings={() => (tab = "settings")}
        />
      </div>
      <div class="view page" hidden={tab !== "statistics"}>
        <StatsView load={api.libraryStats} visible={tab === "statistics"} />
      </div>
      <div class="view page settings" hidden={tab !== "settings"}>
        {#key settingsRevision}
          <SettingsPanel
            onchange={(s) => (settings = s)}
            onsetup={() => (setup = true)}
          />
        {/key}
      </div>
    </main>
    {#if setup}
      <SetupWizard onclose={() => void closeSetup()} />
    {/if}
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

  .status {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 0.9rem;
    min-width: 0;
  }

  .version {
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

  .view.page {
    height: auto;
  }

  .view.settings {
    max-width: 46rem;
  }

  .view[hidden] {
    display: none;
  }
</style>
