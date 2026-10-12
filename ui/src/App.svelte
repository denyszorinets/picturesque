<script lang="ts">
  import { appInfo, type AppInfo } from "./lib/api";

  let info = $state<AppInfo | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    appInfo()
      .then((result) => (info = result))
      .catch((e: unknown) => (error = String(e)));
  });
</script>

<main>
  <h1>Picturesque</h1>
  {#if info}
    <p class="version" data-testid="app-version">
      {info.name}
      {info.version}
    </p>
  {:else if error}
    <p class="error" role="alert">Backend unavailable: {error}</p>
  {:else}
    <p class="muted">Connecting…</p>
  {/if}
</main>

<style>
  main {
    padding: 2rem;
  }

  .version,
  .muted {
    color: var(--muted);
  }

  .error {
    color: #c0392b;
  }
</style>
