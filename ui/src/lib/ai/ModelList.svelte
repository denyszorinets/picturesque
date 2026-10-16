<script lang="ts">
  import * as api from "../api";
  import ExternalLink from "../components/ExternalLink.svelte";
  import { formatSize, Models } from "./models.svelte";

  const models = new Models(api);

  $effect(() => {
    void models.load();
    return api.onAppEvent((event) => models.handle(event));
  });
</script>

<div class="models" data-testid="model-list">
  {#if models.list.length === 0}
    <p class="hint muted">No models are available yet.</p>
  {/if}
  {#each models.list as model (model.id)}
    <div class="model" data-testid="model" data-model-id={model.id}>
      <div class="about">
        <strong>{model.name}</strong>
        <span class="muted">{model.purpose}</span>
        <span class="meta muted">
          {formatSize(model.size)} ·
          <ExternalLink href={model.license_url}>{model.license}</ExternalLink>
          ·
          <ExternalLink href={model.homepage}>Source</ExternalLink>
        </span>
      </div>
      <div class="state" data-testid="model-state" data-state={model.state}>
        {#if model.state === "installed"}
          <span class="ok">Downloaded</span>
          <button class="small" onclick={() => models.remove(model.id)}
            >Remove</button
          >
        {:else if model.state === "downloading"}
          <progress max={model.size} value={model.downloaded}></progress>
          <button class="small" onclick={() => models.cancel(model.id)}
            >Cancel</button
          >
        {:else}
          {#if model.error}
            <span class="error-text" title={model.error}>Failed</span>
          {/if}
          <button class="small" onclick={() => models.install(model.id)}
            >{model.error ? "Retry" : "Download"}</button
          >
        {/if}
      </div>
      {#if model.error}
        <p class="error-text detail" data-testid="model-error">
          {model.error}
        </p>
      {/if}
    </div>
  {/each}
  {#if models.error}
    <p class="error-text" role="alert">{models.error}</p>
  {/if}
</div>

<style>
  .models {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .model {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 0.3rem 1rem;
    padding: 0.6rem 0.8rem;
    border: 1px solid var(--border);
    border-radius: 8px;
  }

  .about {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    font-size: 0.88rem;
  }

  .meta {
    font-size: 0.8rem;
  }

  .state {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.85rem;
  }

  .ok {
    color: var(--accent);
    font-weight: 600;
  }

  .small {
    padding: 0.2rem 0.6rem;
    font-size: 0.85rem;
  }

  .detail {
    grid-column: 1 / -1;
    margin: 0;
    font-size: 0.8rem;
    overflow-wrap: anywhere;
  }

  .hint {
    margin: 0;
    font-size: 0.85rem;
  }
</style>
