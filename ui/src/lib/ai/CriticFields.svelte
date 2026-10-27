<script lang="ts">
  import type { CriticStatus } from "../api";

  interface Props {
    enabled: boolean;
    server: string;
    model: string;
    /** `""` uses the default rubric. */
    rubric: string;
    /** For the saved settings; `null` while unknown. */
    status: CriticStatus | null;
    /** Asks the server again (for the saved settings). */
    oncheck: () => void;
    /** Whether the fields differ from the saved settings. */
    unsaved: boolean;
  }

  let {
    enabled = $bindable(),
    server = $bindable(),
    model = $bindable(),
    rubric = $bindable(),
    status,
    oncheck,
    unsaved,
  }: Props = $props();

  const local = $derived(isLocal(server));
  /** Installed models first, then suggested ones not installed. */
  const choices = $derived([
    ...(status?.models ?? []),
    ...(status?.suggested ?? [])
      .map((s) => s.name)
      .filter((n) => !(status?.models ?? []).includes(n)),
  ]);

  /** Mirrors the server's check (`critic::is_local`) for an instant hint. */
  function isLocal(url: string): boolean {
    let host: string;
    try {
      host = new URL(url).hostname.replace(/^\[|\]$/g, "");
    } catch {
      return true;
    }
    return (
      host.toLowerCase() === "localhost" ||
      host === "::1" ||
      /^127\.\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(host)
    );
  }

  function gigabytes(size: number): string {
    return `${(size / 1e9).toFixed(1)} GB`;
  }
</script>

<div class="critic">
  <label class="inline">
    <input
      type="checkbox"
      bind:checked={enabled}
      data-testid="critic-enabled"
    />
    Critique photos with a local vision-language model
  </label>
  <p class="hint muted">
    The loupe gets a <strong>Critique this photo</strong> button: a rating and
    what works and what does not, from a model on a server you run, such as
    <a href="https://ollama.com" target="_blank" rel="noreferrer">Ollama</a>.
  </p>
  {#if enabled}
    <label class="inline">
      Server
      <input
        type="url"
        class="wide"
        bind:value={server}
        spellcheck="false"
        data-testid="critic-server"
      />
    </label>
    {#if !local}
      <p class="warning" role="note" data-testid="critic-remote">
        This server is not on this computer: photos you critique are sent to it.
      </p>
    {/if}
    <label class="inline">
      Model
      <input
        type="text"
        class="wide"
        list="critic-models"
        bind:value={model}
        spellcheck="false"
        data-testid="critic-model"
      />
      <datalist id="critic-models">
        {#each choices as name (name)}
          <option value={name}></option>
        {/each}
      </datalist>
    </label>
    {#if status && status.suggested.length > 0}
      <ul class="suggested hint muted" data-testid="critic-suggested">
        {#each status.suggested as s (s.name)}
          <li>
            <button
              class="link"
              onclick={() => (model = s.name)}
              title="Use this model">{s.name}</button
            >
            · {gigabytes(s.size)} ·
            <a href={s.license_url} target="_blank" rel="noreferrer"
              >{s.license}</a
            >
            · {s.note}
          </li>
        {/each}
      </ul>
    {/if}
    <label class="field">
      <span>Rubric: what the model judges</span>
      <textarea
        rows="6"
        bind:value={rubric}
        placeholder={status?.default_rubric ?? ""}
        data-testid="critic-rubric"></textarea>
    </label>
    {#if rubric !== ""}
      <button
        class="link"
        onclick={() => (rubric = "")}
        data-testid="critic-rubric-default">Use the default rubric</button
      >
    {/if}
    <p class="hint muted" data-testid="critic-status">
      {#if unsaved}
        Save to use these settings.
      {:else if !status}
        Checking…
      {:else if status.problem}
        <span class="problem">{status.problem}</span>
      {:else if status.reachable}
        Ready: {status.model} is installed.
      {/if}
      {#if !unsaved}
        <button class="link" onclick={oncheck} data-testid="critic-check"
          >Check again</button
        >
      {/if}
    </p>
  {/if}
</div>

<style>
  .critic {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }

  .inline {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.9rem;
  }

  .wide {
    flex: 1;
    min-width: 0;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.9rem;
  }

  textarea {
    font: inherit;
    font-size: 0.85rem;
    resize: vertical;
  }

  .hint {
    margin: 0;
    font-size: 0.85rem;
  }

  .suggested {
    list-style: none;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .warning {
    margin: 0;
    font-size: 0.85rem;
    color: #e0a03a;
  }

  .problem {
    overflow-wrap: anywhere;
  }

  .link {
    border: none;
    background: none;
    padding: 0;
    color: var(--muted);
    font-size: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
    align-self: flex-start;
  }

  .link:hover {
    background: none;
    color: var(--fg);
  }
</style>
