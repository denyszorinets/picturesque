<script lang="ts">
  import * as api from "./lib/api";
  import type { AppInfo, Settings } from "./lib/api";
  import ActivityView from "./lib/activity/ActivityView.svelte";
  import ActivityRing from "./lib/activity/ActivityRing.svelte";
  import { Pipeline } from "./lib/activity/pipeline.svelte";
  import { ringState } from "./lib/activity/ring";
  import { AnalysisProgress } from "./lib/library/analysis.svelte";
  import FolderSwitcher from "./lib/session/FolderSwitcher.svelte";
  import LibraryView from "./lib/library/LibraryView.svelte";
  import SettingsPanel from "./lib/settings/SettingsPanel.svelte";
  import SetupWizard from "./lib/setup/SetupWizard.svelte";
  import StatsView from "./lib/stats/StatsView.svelte";
  import HelpDialog from "./lib/help/HelpDialog.svelte";
  import Icon, { type IconName } from "./lib/components/Icon.svelte";
  import { tourWanted } from "./lib/tour/policy";

  type Tab = "library" | "activity" | "statistics" | "settings";
  const TABS: { id: Tab; label: string; icon: IconName }[] = [
    { id: "library", label: "Library", icon: "photo" },
    { id: "activity", label: "Activity", icon: "arrow-path" },
    { id: "statistics", label: "Statistics", icon: "chart-bar" },
    { id: "settings", label: "Settings", icon: "cog-6-tooth" },
  ];

  let info = $state<AppInfo | null>(null);
  let error = $state<string | null>(null);
  let settings = $state<Settings | null>(null);
  let tab = $state<Tab>("library");
  let help = $state(false);
  /** The setup wizard: opened on first launch, or from Settings. */
  let setup = $state(false);
  /** Bumped after the wizard so Settings shows what it saved. */
  let settingsRevision = $state(0);
  /** Bumped when another folder (or the library) is opened, so every
   * view starts over with that session's photos. */
  let sessionRevision = $state(0);
  /** The folder menu, also opened from Finish folder. */
  let switcherOpen = $state(false);
  const inFolder = $derived(!!settings?.open_folder);
  /** The culling tour, asked for again from Settings. */
  let replayTour = $state(false);
  const tour = $derived(
    tourWanted({
      tourDone: settings ? settings.onboarding.tour_done : null,
      replay: replayTour,
      hasLibrary: !!settings?.library_root,
      setupOpen: setup,
      helpOpen: help,
      onLibraryTab: tab === "library",
    }),
  );

  async function endTour() {
    replayTour = false;
    if (settings) settings = { ...settings, onboarding: { tour_done: true } };
    try {
      settings = await api.updateSettings({ onboarding: { tour_done: true } });
    } catch {
      // Not saved: the tour shows again next time.
    }
  }

  const pipeline = new Pipeline(api);
  const analysis = new AnalysisProgress(api);
  const ring = $derived(ringState(pipeline.status, analysis.status));

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

  /** ? opens Help, except while typing. */
  function onkeydown(e: KeyboardEvent) {
    const target = e.target instanceof Element ? e.target : null;
    if (
      e.key !== "?" ||
      help ||
      setup ||
      e.defaultPrevented ||
      target?.closest("input, select, textarea, [contenteditable]")
    )
      return;
    e.preventDefault();
    help = true;
  }

  /** Another folder or the library was opened. */
  function switched(next: Settings) {
    settings = next;
    setup = false;
    tab = "library";
    sessionRevision += 1;
    void pipeline.loadStatus();
    void analysis.load();
  }

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

  // The header ring follows importing and analysis everywhere.
  $effect(() => {
    void pipeline.loadStatus();
    void analysis.load();
    const stop = api.onAppEvent((event) => {
      pipeline.handle(event);
      analysis.handle(event);
    });
    // Events may be missed (for example while reconnecting).
    const timer = setInterval(() => {
      if (pipeline.busy) void pipeline.loadStatus();
    }, 3000);
    return () => {
      stop();
      clearInterval(timer);
      analysis.dispose();
    };
  });
</script>

<svelte:window {onkeydown} />

<div class="app">
  <header>
    <h1>Picturesque</h1>
    {#if info && settings}
      <FolderSwitcher {settings} onswitch={switched} bind:open={switcherOpen} />
    {/if}
    {#if info}
      <nav aria-label="Main">
        {#each TABS as t (t.id)}
          <button
            class:active={tab === t.id}
            aria-current={tab === t.id ? "page" : undefined}
            onclick={() => (tab = t.id)}
          >
            <Icon name={t.icon} />
            {t.label}
          </button>
        {/each}
      </nav>
      <div class="status">
        <ActivityRing {ring} onactivity={() => (tab = "activity")} />
        <button
          class="help"
          onclick={() => (help = true)}
          title="Keyboard shortcuts and guides (?)"
          data-testid="open-help"
          ><Icon name="question-mark-circle" /> Help</button
        >
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
      {#key sessionRevision}
        <!-- Views stay mounted so they keep their state while switching. -->
        <div class="view" hidden={tab !== "library"}>
          <LibraryView
            onimport={() => (tab = "activity")}
            onsettings={() => (tab = "settings")}
            onsetup={() => (setup = true)}
            hasLibrary={settings === null || !!settings.library_root}
            importing={pipeline.busy}
            {analysis}
            {inFolder}
            autoLabel={settings?.labels.auto_pick ?? null}
            onnextfolder={() => (switcherOpen = true)}
            folder={settings?.open_folder ?? null}
            canClear={settings?.delete_after_copy !== "never"}
            {tour}
            ontourend={() => void endTour()}
          />
        </div>
        <div class="view page" hidden={tab !== "activity"}>
          <ActivityView
            {pipeline}
            visible={tab === "activity"}
            hasLibrary={settings === null || !!settings.library_root}
            {inFolder}
            onsettings={() => (tab = "settings")}
            onsetup={() => (setup = true)}
          />
        </div>
        <div class="view page" hidden={tab !== "statistics"}>
          <StatsView
            load={api.libraryStats}
            visible={tab === "statistics"}
            onimport={() =>
              settings && !settings.library_root
                ? (setup = true)
                : (tab = "activity")}
          />
        </div>
        <div class="view page settings" hidden={tab !== "settings"}>
          {#key settingsRevision}
            <SettingsPanel
              {inFolder}
              onchange={(s) => (settings = s)}
              onsetup={() => (setup = true)}
              ontour={() => {
                replayTour = true;
                tab = "library";
              }}
            />
          {/key}
        </div>
      {/key}
    </main>
    {#if help}
      <HelpDialog onclose={() => (help = false)} />
    {/if}
    {#if setup}
      <SetupWizard onclose={() => void closeSetup()} onopened={switched} />
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
    font-size: 1.2rem;
    letter-spacing: -0.01em;
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

  nav button:hover,
  .help:hover {
    background: var(--hover);
    color: var(--fg);
  }

  nav button.active {
    background: var(--surface-2);
    color: var(--fg);
    box-shadow: inset 0 0 0 1px var(--border);
  }

  .status {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 0.9rem;
    min-width: 0;
  }

  .help {
    border: none;
    background: none;
    padding: 0.35rem 0.6rem;
    color: var(--muted);
    font-weight: 600;
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
