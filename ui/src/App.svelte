<script lang="ts">
  import { appInfo, type AppInfo, type Settings } from "./lib/api";
  import ImportView from "./lib/import/ImportView.svelte";
  import SettingsPanel from "./lib/import/SettingsPanel.svelte";

  let info = $state<AppInfo | null>(null);
  let error = $state<string | null>(null);
  let settings = $state<Settings | null>(null);

  const versionLabel = (i: AppInfo) => `v${i.version}`;

  $effect(() => {
    appInfo()
      .then((result) => (info = result))
      .catch((e: unknown) => (error = String(e)));
  });
</script>

<header>
  <h1>Picturesque</h1>
  {#if info}
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
  <main>
    <aside>
      <SettingsPanel onchange={(s) => (settings = s)} />
    </aside>
    <div class="content">
      <ImportView hasLibrary={!!settings?.library_root} />
    </div>
  </main>
{/if}

<style>
  header {
    display: flex;
    align-items: baseline;
    gap: 0.8rem;
    padding: 1rem 1.5rem 0.5rem;
  }

  header p {
    margin: 0;
  }

  .version {
    color: var(--muted);
    font-size: 0.85rem;
  }

  .error {
    color: var(--danger);
  }

  main {
    display: grid;
    grid-template-columns: minmax(18rem, 24rem) minmax(0, 1fr);
    gap: 1.2rem;
    padding: 0.8rem 1.5rem 2rem;
    align-items: start;
  }

  @media (max-width: 900px) {
    main {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
