<script lang="ts">
  import type { JobFilter, LogLevel } from "../api";
  import PathInput from "../components/PathInput.svelte";
  import { STEP_LABELS, steps, type Pipeline } from "./pipeline.svelte";

  interface Props {
    pipeline: Pipeline;
    /** Whether the view is on screen; it reloads only then. */
    visible: boolean;
    /** Whether a library folder is set; importing waits for one. */
    hasLibrary: boolean;
    onsettings?: () => void;
  }

  let { pipeline, visible, hasLibrary, onsettings }: Props = $props();
  let draft = $state("");

  /** How often the view reloads while visible. */
  const REFRESH_MS = 1500;

  $effect(() => {
    if (!visible) return;
    void pipeline.refresh();
    const timer = setInterval(() => void pipeline.refresh(), REFRESH_MS);
    return () => clearInterval(timer);
  });

  async function add(path: string) {
    if (await pipeline.addSource(path)) draft = "";
  }

  const status = $derived(pipeline.status);
  const filters = $derived<{ id: JobFilter; label: string; count?: number }[]>([
    { id: "all", label: "All" },
    { id: "active", label: "In progress", count: status?.queued },
    { id: "imported", label: "Imported", count: status?.imported },
    { id: "duplicate", label: "Duplicates", count: status?.duplicates },
    { id: "failed", label: "Failed", count: status?.failed },
  ]);
  const LEVELS: { id: LogLevel; label: string }[] = [
    { id: "info", label: "Everything" },
    { id: "warn", label: "Warnings and errors" },
    { id: "error", label: "Errors" },
  ];
  const STATE_TEXT = {
    done: "done",
    active: "in progress",
    pending: "waiting",
    skipped: "skipped",
    failed: "failed",
  } as const;

  function time(iso: string | null): string {
    if (!iso) return "";
    const at = new Date(iso);
    if (Number.isNaN(at.getTime())) return iso;
    const today = new Date().toDateString() === at.toDateString();
    return today
      ? at.toLocaleTimeString()
      : `${at.toLocaleDateString()} ${at.toLocaleTimeString()}`;
  }

  function outcome(job: (typeof pipeline.jobs)[number]): string {
    switch (job.stage) {
      case "queued":
        return "Waiting";
      case "reading":
        return "Reading metadata";
      case "copying":
        return "Copying and verifying";
      case "imported":
        return "In the library";
      case "duplicate":
        return job.error ?? "Already in the library";
      case "failed":
        return job.error ?? "Failed";
    }
  }
</script>

<div class="activity">
  <section class="card sources" aria-labelledby="sources-title">
    <div class="heading">
      <h2 id="sources-title">Import folders</h2>
      {#if pipeline.sources.length > 0}
        <button
          class="small"
          onclick={() => pipeline.scan()}
          disabled={status?.scanning}
          data-testid="scan-all">Check now</button
        >
      {/if}
    </div>
    <p class="muted hint">
      Photos in these folders and their subfolders are copied into the library.
      Watched folders are imported automatically when new photos arrive. The
      folders themselves are never changed.
    </p>

    {#if !hasLibrary}
      <p class="notice" data-testid="needs-library">
        Choose a library folder in
        <button class="link" onclick={() => onsettings?.()}>Settings</button>
        first; photos wait until then.
      </p>
    {:else if status?.paused}
      <p class="notice" data-testid="paused">{status.paused}</p>
    {/if}

    {#if pipeline.sources.length > 0}
      <ul class="source-list" data-testid="sources">
        {#each pipeline.sources as source (source.id)}
          <li>
            <div class="source-main">
              <code>{source.path}</code>
              {#if source.scan_error}
                <span class="error-text small-text">{source.scan_error}</span>
              {:else if source.scanned_at}
                <span class="muted small-text"
                  >Checked {time(source.scanned_at)}</span
                >
              {:else}
                <span class="muted small-text">Not checked yet</span>
              {/if}
            </div>
            <label class="watch" title="Import new photos automatically">
              <input
                type="checkbox"
                checked={source.watch}
                onchange={(e) =>
                  pipeline.setWatch(source.id, e.currentTarget.checked)}
                data-testid="source-watch"
              />
              Watch
            </label>
            <button
              class="small"
              onclick={() => pipeline.scan(source.id)}
              aria-label={`Check ${source.path} now`}>Check</button
            >
            <button
              class="remove"
              onclick={() => pipeline.removeSource(source.id)}
              aria-label={`Remove ${source.path}`}
              title="Stop importing from this folder (imported photos stay)"
              >✕</button
            >
          </li>
        {/each}
      </ul>
    {/if}

    <div class="add">
      <PathInput
        label="Folder to import from"
        bind:value={draft}
        multiple
        onpick={(paths) => paths.forEach((p) => void add(p))}
        onsubmit={(p) => void add(p)}
        testid="source-path"
      />
      <button
        class="primary"
        onclick={() => add(draft)}
        disabled={!draft.trim()}
        data-testid="add-source">Add folder</button
      >
    </div>
    {#if pipeline.error}
      <p class="error-text" role="alert" data-testid="activity-error">
        {pipeline.error}
      </p>
    {/if}
  </section>

  <section class="card photos" aria-labelledby="photos-title">
    <div class="heading">
      <h2 id="photos-title">Photos</h2>
      {#if status?.active}
        <span class="current" data-testid="current-photo">
          <span class="spinner" aria-hidden="true"></span>
          {status.active.stage === "reading" ? "Reading" : "Copying"}
          {status.active.file_name}
        </span>
      {:else if status?.scanning}
        <span class="current">
          <span class="spinner" aria-hidden="true"></span>
          Looking for new photos
        </span>
      {/if}
    </div>

    <div class="toolbar">
      <div class="filters" role="group" aria-label="Show photos">
        {#each filters as f (f.id)}
          <button
            class="small"
            aria-pressed={pipeline.filter === f.id}
            onclick={() => pipeline.setFilter(f.id)}
            data-testid={`filter-${f.id}`}
          >
            {f.label}{#if f.count !== undefined}<span class="count"
                >{f.count}</span
              >{/if}
          </button>
        {/each}
      </div>
      {#if (status?.failed ?? 0) > 0}
        <button
          class="small"
          onclick={() => pipeline.retry()}
          data-testid="retry-failed">Retry failed</button
        >
      {/if}
    </div>

    {#if pipeline.jobs.length === 0}
      <p class="muted empty" data-testid="no-jobs">
        {pipeline.sources.length === 0
          ? "Add a folder to start importing."
          : "No photos here."}
      </p>
    {:else}
      <div class="table-wrap">
        <table data-testid="jobs">
          <thead>
            <tr>
              <th scope="col">Photo</th>
              {#each STEP_LABELS as label (label)}
                <th scope="col" class="step-head"><span>{label}</span></th>
              {/each}
              <th scope="col">Status</th>
            </tr>
          </thead>
          <tbody>
            {#each pipeline.jobs as job (job.id)}
              <tr data-stage={job.stage} data-testid="job">
                <td class="name" title={job.source_path}>
                  <span class="file">{job.file_name}</span>
                  {#if job.files.length > 1}
                    <span class="muted small-text extra"
                      >+{job.files
                        .filter((f) => f !== job.file_name)
                        .join(", ")}</span
                    >
                  {/if}
                </td>
                {#each steps(job) as step (step.label)}
                  <td
                    class="step"
                    title={`${step.label}: ${STATE_TEXT[step.state]}${step.detail ? ` (${step.detail})` : ""}`}
                  >
                    <span
                      class={`dot ${step.state}`}
                      role="img"
                      aria-label={`${step.label}: ${STATE_TEXT[step.state]}`}
                    ></span>
                  </td>
                {/each}
                <td
                  class="outcome"
                  class:error-text={job.stage === "failed"}
                  data-testid="job-outcome"
                >
                  {outcome(job)}
                  <span class="muted small-text">{time(job.updated_at)}</span>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      {#if !pipeline.jobsComplete}
        <button class="small more" onclick={() => pipeline.moreJobs()}
          >Show more</button
        >
      {/if}
    {/if}
  </section>

  <section class="card log" aria-labelledby="log-title">
    <div class="heading">
      <h2 id="log-title">Log</h2>
      <select
        aria-label="Log level"
        value={pipeline.logLevel}
        onchange={(e) =>
          pipeline.setLogLevel(e.currentTarget.value as LogLevel)}
        data-testid="log-level"
      >
        {#each LEVELS as level (level.id)}
          <option value={level.id}>{level.label}</option>
        {/each}
      </select>
    </div>
    {#if pipeline.log.length === 0}
      <p class="muted empty">Nothing yet.</p>
    {:else}
      <ol class="entries" data-testid="log">
        {#each pipeline.log as entry (entry.id)}
          <li class={entry.level}>
            <time datetime={entry.at}>{time(entry.at)}</time>
            <span class="message">{entry.message}</span>
          </li>
        {/each}
      </ol>
      {#if !pipeline.logComplete}
        <button class="small more" onclick={() => pipeline.moreLog()}
          >Show older</button
        >
      {/if}
    {/if}
  </section>
</div>

<style>
  .activity {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(18rem, 26rem);
    grid-template-areas:
      "sources log"
      "photos log";
    grid-template-rows: auto 1fr;
    gap: 1rem;
    align-items: start;
  }

  .sources {
    grid-area: sources;
  }

  .photos {
    grid-area: photos;
  }

  .log {
    grid-area: log;
    position: sticky;
    top: 0;
    max-height: calc(100vh - 6rem);
    display: flex;
    flex-direction: column;
  }

  @media (max-width: 1100px) {
    .activity {
      grid-template-columns: minmax(0, 1fr);
      grid-template-areas: "sources" "photos" "log";
    }

    .log {
      position: static;
      max-height: 28rem;
    }
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
    min-width: 0;
  }

  .heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.8rem;
  }

  h2 {
    font-size: 1.05rem;
  }

  p {
    margin: 0;
  }

  .hint,
  .small-text {
    font-size: 0.82rem;
  }

  button.small {
    padding: 0.25rem 0.65rem;
    font-size: 0.85rem;
  }

  button.link {
    border: none;
    background: none;
    padding: 0;
    color: var(--accent);
    text-decoration: underline;
  }

  .notice {
    padding: 0.5rem 0.7rem;
    border-radius: 8px;
    background: color-mix(in srgb, var(--warn) 15%, transparent);
    font-size: 0.9rem;
  }

  .source-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .source-list li {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.45rem 0.6rem;
    background: var(--surface-2);
    border-radius: 8px;
  }

  .source-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    overflow-wrap: anywhere;
  }

  .watch {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.85rem;
    white-space: nowrap;
  }

  .remove {
    border: none;
    background: none;
    padding: 0.1rem 0.4rem;
    color: var(--muted);
  }

  .add {
    display: flex;
    gap: 0.5rem;
  }

  .add :global(.path-input) {
    flex: 1;
  }

  .current {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    color: var(--muted);
    font-size: 0.85rem;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .spinner {
    flex: none;
    width: 0.8rem;
    height: 0.8rem;
    border-radius: 50%;
    border: 2px solid var(--border);
    border-top-color: var(--info);
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }

  .filters button[aria-pressed="true"] {
    background: var(--surface-2);
    border-color: var(--muted);
    font-weight: 600;
  }

  .count {
    margin-left: 0.35rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .empty {
    padding: 1rem 0;
    text-align: center;
  }

  .table-wrap {
    overflow-x: auto;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.88rem;
  }

  th {
    text-align: left;
    font-weight: 600;
    color: var(--muted);
    font-size: 0.78rem;
    padding: 0.3rem 0.4rem;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }

  th.step-head {
    text-align: center;
    width: 4.2rem;
  }

  td {
    padding: 0.35rem 0.4rem;
    border-bottom: 1px solid var(--border);
    vertical-align: middle;
  }

  td.name {
    min-width: 13rem;
    overflow-wrap: anywhere;
  }

  td.name .extra {
    display: block;
  }

  .file {
    font-weight: 600;
  }

  td.step {
    text-align: center;
  }

  td.outcome {
    min-width: 11rem;
    overflow-wrap: anywhere;
  }

  td.outcome .small-text {
    display: block;
  }

  .dot {
    display: inline-block;
    width: 0.75rem;
    height: 0.75rem;
    border-radius: 50%;
    border: 2px solid var(--border);
  }

  .dot.done {
    background: var(--accent);
    border-color: var(--accent);
  }

  .dot.active {
    border-color: var(--info);
    background: color-mix(in srgb, var(--info) 35%, transparent);
    animation: pulse 1.2s ease-in-out infinite;
  }

  .dot.failed {
    background: var(--danger);
    border-color: var(--danger);
  }

  .dot.skipped {
    border-style: dashed;
    opacity: 0.5;
  }

  @keyframes pulse {
    50% {
      opacity: 0.4;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .dot.active,
    .spinner {
      animation: none;
    }
  }

  .more {
    align-self: center;
  }

  .entries {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    font-size: 0.84rem;
    display: flex;
    flex-direction: column;
  }

  .entries li {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 0.6rem;
    padding: 0.3rem 0.2rem 0.3rem 0.5rem;
    border-left: 3px solid transparent;
    border-bottom: 1px solid var(--border);
  }

  .entries li.warn {
    border-left-color: var(--warn);
  }

  .entries li.error {
    border-left-color: var(--danger);
  }

  .entries time {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .message {
    overflow-wrap: anywhere;
  }
</style>
