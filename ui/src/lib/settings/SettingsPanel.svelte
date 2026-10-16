<script lang="ts">
  import {
    aiStatus,
    ApiError,
    getSettings,
    previewTemplate,
    reanalyzePhotos,
    templatePresets,
    updateSettings,
    type AiStatus,
    type Provider,
    type ProviderChoice,
    type Settings,
    type SettingsUpdate,
    type TemplatePresetView,
    type TemplatePreview,
  } from "../api";
  import ModelList from "../ai/ModelList.svelte";
  import Reanalyze from "../analysis/Reanalyze.svelte";
  import PathInput from "../components/PathInput.svelte";

  interface Props {
    /** Called whenever settings are loaded or saved. */
    onchange?: (settings: Settings) => void;
    /** Opens the setup wizard. */
    onsetup?: () => void;
  }

  let { onchange, onsetup }: Props = $props();

  let saved = $state<Settings | null>(null);
  let presets = $state<TemplatePresetView[]>([]);
  let libraryRoot = $state("");
  let template = $state("");
  let sidecarNaming = $state<Settings["sidecar_naming"]>("file_extension");
  let maxGap = $state(30);
  let maxDistance = $state(16);
  let rescanMinutes = $state(5);
  let autoPick = $state(true);
  /** Minimum quality for auto-pick, in percent. */
  let autoPickPercent = $state(50);
  let weights = $state({
    sharpness: 0.6,
    exposure: 0.3,
    noise: 0.1,
    aesthetic: 0.4,
  });
  const WEIGHT_LABELS = {
    sharpness: "Sharpness",
    exposure: "Exposure",
    noise: "Low noise",
    aesthetic: "Aesthetic",
  } as const;
  let provider = $state<ProviderChoice>("auto");
  let ai = $state<AiStatus | null>(null);
  const PROVIDER_LABELS: Record<Provider, string> = {
    cpu: "CPU",
    directml: "DirectML",
    coreml: "Core ML",
    webgpu: "WebGPU",
  };
  const providers = $derived<Provider[]>(ai?.available ?? ["cpu"]);
  let preview = $state<TemplatePreview | null>(null);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let saving = $state(false);

  const presetId = $derived(
    presets.find((p) => p.template === template.trim())?.id ?? "custom",
  );
  const seriesChanged = $derived(
    saved !== null &&
      (maxGap !== saved.series.max_gap_seconds ||
        maxDistance !== saved.series.max_distance),
  );
  const rankingChanged = $derived(
    saved !== null &&
      (weights.sharpness !== saved.ranking.sharpness ||
        weights.exposure !== saved.ranking.exposure ||
        weights.noise !== saved.ranking.noise ||
        weights.aesthetic !== saved.ranking.aesthetic),
  );
  const autoPickChanged = $derived(
    saved !== null &&
      (autoPick !== saved.auto_pick.enabled ||
        autoPickPercent !== Math.round(saved.auto_pick.min_quality * 100)),
  );
  const dirty = $derived(
    saved !== null &&
      (libraryRoot.trim() !== (saved.library_root ?? "") ||
        template.trim() !== saved.path_template ||
        sidecarNaming !== saved.sidecar_naming ||
        seriesChanged ||
        rankingChanged ||
        rescanMinutes !== saved.import.rescan_minutes ||
        autoPickChanged ||
        provider !== saved.ai.provider),
  );

  $effect(() => {
    Promise.all([getSettings(), templatePresets()])
      .then(([settings, list]) => {
        presets = list;
        apply(settings);
      })
      .catch((e: unknown) => (error = ApiError.from(e).message));
  });

  /** Probes the provider; slow the first time (it loads a test model). */
  function loadAi() {
    aiStatus()
      .then((status) => (ai = status))
      .catch((e: unknown) => (error = ApiError.from(e).message));
  }

  // Live preview, debounced while typing.
  $effect(() => {
    const current = template;
    const timer = setTimeout(() => {
      previewTemplate(current)
        .then((p) => {
          if (current === template) preview = p;
        })
        .catch(() => {});
    }, 200);
    return () => clearTimeout(timer);
  });

  function apply(settings: Settings) {
    saved = settings;
    libraryRoot = settings.library_root ?? "";
    template = settings.path_template;
    sidecarNaming = settings.sidecar_naming;
    maxGap = settings.series.max_gap_seconds;
    maxDistance = settings.series.max_distance;
    rescanMinutes = settings.import.rescan_minutes;
    autoPick = settings.auto_pick.enabled;
    autoPickPercent = Math.round(settings.auto_pick.min_quality * 100);
    weights = { ...settings.ranking };
    provider = settings.ai.provider;
    loadAi();
    onchange?.(settings);
  }

  function choosePreset(id: string) {
    const preset = presets.find((p) => p.id === id);
    if (preset) template = preset.template;
  }

  async function save() {
    if (!saved) return;
    saving = true;
    error = null;
    notice = null;
    try {
      const update: SettingsUpdate = {};
      if (seriesChanged)
        update.series = { max_gap_seconds: maxGap, max_distance: maxDistance };
      if (rankingChanged) update.ranking = { ...weights };
      if (provider !== saved.ai.provider) update.ai = { provider };
      if (autoPickChanged)
        update.auto_pick = {
          enabled: autoPick,
          min_quality: autoPickPercent / 100,
        };
      if (rescanMinutes !== saved.import.rescan_minutes)
        update.import = { rescan_minutes: rescanMinutes };
      if (sidecarNaming !== saved.sidecar_naming)
        update.sidecar_naming = sidecarNaming;
      if (libraryRoot.trim() !== (saved.library_root ?? ""))
        update.library_root = libraryRoot.trim();
      if (template.trim() !== saved.path_template)
        update.path_template = template.trim();
      apply(await updateSettings(update));
      notice = "Saved.";
    } catch (e) {
      error = ApiError.from(e).message;
    } finally {
      saving = false;
    }
  }
</script>

<section class="card settings" aria-labelledby="settings-title">
  <div class="title">
    <h2 id="settings-title">Settings</h2>
    {#if onsetup}
      <button class="small" onclick={onsetup} data-testid="open-setup"
        >Run setup again</button
      >
    {/if}
  </div>

  <div class="field">
    <span class="label">Library folder (output)</span>
    <PathInput
      label="Library folder"
      bind:value={libraryRoot}
      testid="library-root"
    />
    <p class="hint muted">
      Imported photos are <strong>copied</strong> here. Your originals are only read:
      never moved, renamed, edited or deleted.
    </p>
  </div>

  <div class="field">
    <label class="label" for="preset">Folder structure</label>
    <select
      id="preset"
      value={presetId}
      onchange={(e) => choosePreset(e.currentTarget.value)}
    >
      {#each presets as preset (preset.id)}
        <option value={preset.id}>{preset.label}</option>
      {/each}
      <option value="custom" disabled={presetId !== "custom"}>Custom</option>
    </select>
    <input
      type="text"
      aria-label="Path template"
      bind:value={template}
      spellcheck="false"
      data-testid="path-template"
    />

    {#if preview?.error}
      <p class="error-text" role="alert" data-testid="template-error">
        {preview.error}
      </p>
    {:else if preview}
      <ul class="examples" data-testid="template-preview">
        {#each preview.examples as example (example.description)}
          <li>
            <span class="muted">{example.description}</span>
            {#each example.paths as path (path)}
              <code>{path}</code>
            {/each}
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  <div class="field">
    <label class="inline">
      <span class="label">Check watched import folders every</span>
      <input
        type="number"
        min="0"
        max="1440"
        bind:value={rescanMinutes}
        data-testid="rescan-minutes"
      />
      minutes
    </label>
    <p class="hint muted">
      Folders also report new photos as they arrive; this catches what they
      miss, such as network shares. 0 checks only on changes and at start.
    </p>
  </div>

  <div class="field">
    <label class="label" for="sidecar-naming">New XMP sidecars</label>
    <select
      id="sidecar-naming"
      bind:value={sidecarNaming}
      data-testid="sidecar-naming"
    >
      <option value="file_extension"
        >IMG_0042.CR3.xmp (darktable, digiKam)</option
      >
      <option value="base_name">IMG_0042.xmp (Lightroom, Bridge)</option>
    </select>
    <p class="hint muted">
      Picks, rejects and ratings are written to sidecars so other photo tools
      see them.
    </p>
  </div>

  <fieldset class="field series">
    <legend class="label">Series</legend>
    <label class="inline">
      Longest pause between shots
      <input
        type="number"
        min="1"
        max="3600"
        bind:value={maxGap}
        data-testid="series-max-gap"
      />
      seconds
    </label>
    <label class="inline">
      Similarity limit
      <input
        type="number"
        min="0"
        max="32"
        bind:value={maxDistance}
        data-testid="series-max-distance"
      />
      bits (lower is stricter)
    </label>
    <p class="hint muted">
      Photos taken within this pause that look alike form a series.
    </p>
    <span class="label">Best photo of a series</span>
    <div class="weights">
      {#each ["sharpness", "exposure", "noise", "aesthetic"] as const as key (key)}
        <label class="inline">
          {WEIGHT_LABELS[key]}
          <input
            type="number"
            min="0"
            step="0.1"
            bind:value={weights[key]}
            data-testid={`ranking-${key}`}
          />
        </label>
      {/each}
    </div>
    <p class="hint muted">
      How much each score counts when choosing the best photo; only the ratios
      matter.
    </p>
    <span class="label">Auto-pick</span>
    <label class="inline">
      <input
        type="checkbox"
        bind:checked={autoPick}
        data-testid="auto-pick-enabled"
      />
      Mark keepers with quality of at least
      <input
        type="number"
        min="0"
        max="100"
        step="5"
        bind:value={autoPickPercent}
        disabled={!autoPick}
        data-testid="auto-pick-quality"
      />
      %
    </label>
    <p class="hint muted">
      The best photo of each series (or a photo in none) with nobody's eyes
      closed is shown as <em>Auto-picked</em> until you pick or reject it. It is only
      a suggestion: it is not written to sidecars or exported as a pick.
    </p>
  </fieldset>

  <div class="field">
    <span class="label">Analysis</span>
    <p class="hint muted">
      Photos are analyzed once, after import. Analyze them again after a failed
      analysis, or to get scores from new models. Earlier results are used until
      the new ones are ready.
    </p>
    <Reanalyze
      reanalyze={reanalyzePhotos}
      label="Re-analyze all photos"
      confirm="Analyze every photo in the library again? This runs in the background and can take a while."
    />
  </div>

  <div class="field">
    <label class="label" for="ai-provider">AI acceleration</label>
    <select id="ai-provider" bind:value={provider} data-testid="ai-provider">
      <option value="auto">Automatic</option>
      {#each providers as p (p)}
        <option value={p}
          >{p === "cpu" ? "CPU only" : PROVIDER_LABELS[p]}</option
        >
      {/each}
    </select>
    {#if ai}
      <p class="hint muted" data-testid="ai-status">
        {#if ai.provider}
          Models run on {PROVIDER_LABELS[ai.provider]}.
        {:else}
          Local AI is not working on this computer.
        {/if}
        {#if ai.problem}
          <span class="problem" title={ai.problem}>{ai.problem}</span>
        {/if}
      </p>
    {:else}
      <p class="hint muted">Checking…</p>
    {/if}
  </div>

  <div class="field">
    <span class="label">AI models</span>
    <p class="hint muted">
      Downloaded once when you ask, then used offline. Each model's license is
      linked.
    </p>
    <ModelList />
  </div>

  <div class="actions">
    <button
      class="primary"
      onclick={save}
      disabled={!dirty || saving || !!preview?.error}
      data-testid="save-settings"
    >
      Save
    </button>
    {#if error}
      <span class="error-text" role="alert" data-testid="settings-error"
        >{error}</span
      >
    {:else if notice && !dirty}
      <span class="muted">{notice}</span>
    {/if}
  </div>
</section>

<style>
  .title {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  .title .small {
    padding: 0.2rem 0.6rem;
    font-size: 0.85rem;
  }

  .settings {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }

  .label {
    font-weight: 600;
    font-size: 0.9rem;
  }

  .hint {
    margin: 0;
    font-size: 0.85rem;
  }

  .problem {
    display: block;
    margin-top: 0.2rem;
    overflow-wrap: anywhere;
  }

  .examples {
    list-style: none;
    margin: 0;
    padding: 0.6rem 0.7rem;
    background: var(--surface-2);
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    font-size: 0.85rem;
  }

  .examples li {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    overflow-wrap: anywhere;
  }

  .series {
    border: none;
    margin: 0;
    padding: 0;
  }

  .inline {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.9rem;
  }

  .weights {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem 1rem;
  }

  .inline input {
    width: 5rem;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 0.8rem;
  }
</style>
