<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import Critique from "./Critique.svelte";
  import {
    ApiError,
    canReveal,
    criticStatus,
    critiquePhoto,
    getCritique,
    getPhoto,
    type CriticStatus,
    revealInFolder,
    type PairView,
    type PhotoDetail,
  } from "../api";
  import { isPair, pairSetting, photoUrl, viewLabel } from "./pair-view.svelte";
  import { joinPath } from "./sidecars.svelte";
  import type { LibraryStore } from "./store.svelte";

  type Flag = "pick" | "reject" | "unflagged";

  interface Props {
    store: LibraryStore;
    /** Index of the shown photo in the timeline list. */
    index: number;
    onnavigate: (index: number) => void;
    onclose: () => void;
    /** Flags or rates the shown photo (the same as the P/X/U and 0–5 keys). */
    onflag?: (flag: Flag) => void;
    onrate?: (rating: number) => void;
    /** Move to the next photo after every decision. */
    autoAdvance?: boolean;
    /** The library folder, to show files in the file manager. */
    libraryFolder?: string | null;
  }

  let {
    store,
    index,
    onnavigate,
    onclose,
    onflag,
    onrate,
    autoAdvance = $bindable(false),
    libraryFolder = null,
  }: Props = $props();

  /** Runs a toolbar action and gives the keyboard back to the shortcuts. */
  function act(e: Event, action: () => void) {
    (e.currentTarget as HTMLElement).blur();
    action();
  }

  let zoomed = $state(false);
  let showInfo = $state(true);
  let detail = $state<PhotoDetail | null>(null);
  let detailError = $state<string | null>(null);
  let fullLoaded = $state(false);
  let scroller: HTMLDivElement | undefined = $state();
  /** Where in the photo (0–1) to centre when zooming in. */
  let focusPoint = { x: 0.5, y: 0.5 };
  let drag: { x: number; y: number; left: number; top: number } | null = null;

  const photo = $derived(store.photos[index]);
  /** Whether critiques are on; checked once per loupe. */
  let critic = $state<CriticStatus | null>(null);
  $effect(() => {
    criticStatus()
      .then((s) => (critic = s))
      .catch(() => {});
  });
  const PAIR_BUTTONS: [PairView, string][] = [
    ["image", "Show the camera's JPEG (R)"],
    ["raw", "Show the RAW (R)"],
  ];
  /** The file of a RAW+JPEG pair shown; kept while moving through photos. */
  let view = $derived<PairView>(pairSetting.view);
  /** The shown file could not be rendered. */
  let broken = $state(false);

  $effect(() => {
    void store.ensureLoaded(Math.min(store.total - 1, index + 1));
  });

  // Details for the info panel.
  $effect(() => {
    const id = photo?.id;
    detail = null;
    detailError = null;
    if (id === undefined) return;
    getPhoto(id)
      .then((d) => {
        if (photo?.id === id) detail = d;
      })
      .catch((e: unknown) => (detailError = ApiError.from(e).message));
  });

  // A new photo starts fitted.
  $effect(() => {
    void index;
    zoomed = false;
    fullLoaded = false;
  });

  $effect(() => {
    void [index, view];
    broken = false;
  });

  // Preload neighbours so moving through photos feels instant.
  $effect(() => {
    for (const i of [index - 1, index + 1]) {
      const neighbour = store.photos[i];
      if (neighbour) new Image().src = photoUrl(neighbour, "preview", view);
    }
  });

  const percent = (score: number) => `${Math.round(score * 100)}%`;

  function go(delta: number) {
    const next = index + delta;
    if (next >= 0 && next < store.total) onnavigate(next);
  }

  function toggleZoom(point?: { x: number; y: number }) {
    focusPoint = point ?? { x: 0.5, y: 0.5 };
    zoomed = !zoomed;
  }

  /** Shows the other file of a pair; while zoomed, the same spot stays in view. */
  function toggleView() {
    if (!photo || !isPair(photo)) return;
    if (zoomed && fullLoaded && scroller && scroller.scrollWidth > 0) {
      focusPoint = {
        x:
          (scroller.scrollLeft + scroller.clientWidth / 2) /
          scroller.scrollWidth,
        y:
          (scroller.scrollTop + scroller.clientHeight / 2) /
          scroller.scrollHeight,
      };
    }
    view = view === "raw" ? "image" : "raw";
  }

  function onFullLoad(e: Event) {
    fullLoaded = true;
    const img = e.currentTarget as HTMLImageElement;
    if (!scroller) return;
    scroller.scrollLeft =
      img.naturalWidth * focusPoint.x - scroller.clientWidth / 2;
    scroller.scrollTop =
      img.naturalHeight * focusPoint.y - scroller.clientHeight / 2;
  }

  function clickFitted(e: MouseEvent) {
    const img = e.currentTarget as HTMLImageElement;
    const rect = img.getBoundingClientRect();
    toggleZoom({
      x: (e.clientX - rect.left) / rect.width,
      y: (e.clientY - rect.top) / rect.height,
    });
  }

  function onpointerdown(e: PointerEvent) {
    if (!scroller) return;
    drag = {
      x: e.clientX,
      y: e.clientY,
      left: scroller.scrollLeft,
      top: scroller.scrollTop,
    };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onpointermove(e: PointerEvent) {
    if (!drag || !scroller) return;
    scroller.scrollLeft = drag.left - (e.clientX - drag.x);
    scroller.scrollTop = drag.top - (e.clientY - drag.y);
  }

  function onpointerup(e: PointerEvent) {
    const moved =
      drag && Math.hypot(e.clientX - drag.x, e.clientY - drag.y) > 4;
    drag = null;
    if (!moved) zoomed = false;
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    const actions: Record<string, () => void> = {
      ArrowRight: () => go(1),
      ArrowLeft: () => go(-1),
      Escape: () => (zoomed ? (zoomed = false) : onclose()),
      z: () => toggleZoom(),
      Z: () => toggleZoom(),
      i: () => (showInfo = !showInfo),
      I: () => (showInfo = !showInfo),
      r: toggleView,
      R: toggleView,
    };
    const action = actions[e.key];
    if (action) {
      e.preventDefault();
      e.stopPropagation();
      action();
    }
  }

  function bytes(n: number): string {
    return n >= 1e6
      ? `${(n / 1e6).toFixed(1)} MB`
      : `${Math.max(1, Math.round(n / 1e3))} KB`;
  }
</script>

<svelte:window {onkeydown} />

<div
  class="loupe"
  role="dialog"
  aria-modal="true"
  aria-label="Photo"
  data-testid="loupe"
>
  <div class="stage">
    {#if photo}
      <!-- A new element per photo: a reused <img> keeps showing the previous
           photo until the next one has loaded. -->
      {#key photo.id}
        {#if zoomed}
          <div
            class="scroller"
            bind:this={scroller}
            role="presentation"
            {onpointerdown}
            {onpointermove}
            {onpointerup}
            data-testid="loupe-zoomed"
          >
            {#if !fullLoaded}<p class="loading">Loading full size…</p>{/if}
            <img
              class="full"
              class:hidden={!fullLoaded}
              src={photoUrl(photo, "full", view)}
              alt={photo.file_name}
              draggable="false"
              onload={onFullLoad}
            />
          </div>
        {:else}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
          <img
            class="fitted"
            class:gone={broken}
            src={photoUrl(photo, "preview", view)}
            alt={photo.file_name}
            draggable="false"
            onclick={clickFitted}
            onerror={() => (broken = true)}
            data-testid="loupe-image"
          />
          {#if broken}
            <p class="loading" data-testid="loupe-broken">
              Can't show the {viewLabel(photo, view)}{isPair(photo)
                ? `. Press R for the ${viewLabel(photo, view === "raw" ? "image" : "raw")}.`
                : "."}
            </p>
          {/if}
        {/if}
      {/key}
    {:else}
      <p class="loading">Loading…</p>
    {/if}

    <div class="toolbar">
      <button
        onclick={() => go(-1)}
        disabled={index === 0}
        aria-label="Previous photo"><Icon name="chevron-left" /></button
      >
      <span data-testid="loupe-position">{index + 1} / {store.total}</span>
      <button
        onclick={() => go(1)}
        disabled={index >= store.total - 1}
        aria-label="Next photo"><Icon name="chevron-right" /></button
      >
      <button onclick={() => toggleZoom()} aria-pressed={zoomed}
        ><Icon name="magnifying-glass-plus" /> {zoomed ? "Fit" : "100%"}</button
      >
      <button onclick={() => (showInfo = !showInfo)} aria-pressed={showInfo}
        ><Icon name="information-circle" /> Info</button
      >
      {#if photo && isPair(photo)}
        <span
          class="pair"
          role="group"
          aria-label="File shown"
          data-testid="loupe-pair"
        >
          {#each PAIR_BUTTONS as [v, title] (v)}
            <button
              aria-pressed={view === v}
              {title}
              onclick={(e) => act(e, () => view !== v && toggleView())}
              data-testid={`loupe-show-${v}`}>{viewLabel(photo, v)}</button
            >
          {/each}
        </span>
      {:else if photo}
        <span
          class="format"
          title="This photo has only this file"
          data-testid="loupe-format">{viewLabel(photo, view)}</span
        >
      {/if}
      <button onclick={onclose} aria-label="Close"
        ><Icon name="x-mark" /></button
      >
    </div>

    {#if photo}
      <div class="toolbar cull" role="group" aria-label="Decision">
        {#if photo.keeper && photo.flag === "unflagged"}
          <span
            class="auto"
            title="The app suggests keeping this photo: P makes it a pick, X rejects it"
            data-testid="loupe-auto-picked">Auto-picked</span
          >
        {/if}
        <button
          class="pick"
          aria-pressed={photo.flag === "pick"}
          title="Pick (P)"
          onclick={(e) => act(e, () => onflag?.("pick"))}
          data-testid="loupe-pick"><Icon name="check" /> Pick</button
        >
        <button
          class="reject"
          aria-pressed={photo.flag === "reject"}
          title="Reject (X)"
          onclick={(e) => act(e, () => onflag?.("reject"))}
          data-testid="loupe-reject"><Icon name="x-mark" /> Reject</button
        >
        <button
          title="Unflag (U)"
          disabled={photo.flag === "unflagged"}
          onclick={(e) => act(e, () => onflag?.("unflagged"))}
          data-testid="loupe-unflag">Unflag</button
        >
        <span class="stars" data-testid="loupe-stars">
          {#each [1, 2, 3, 4, 5] as n (n)}
            <button
              class="star"
              class:on={photo.rating >= n}
              aria-label={`${n} star${n === 1 ? "" : "s"}`}
              aria-pressed={photo.rating === n}
              title={photo.rating === n
                ? `Clear rating (0)`
                : `${n} star${n === 1 ? "" : "s"} (${n})`}
              onclick={(e) =>
                act(e, () => onrate?.(photo.rating === n ? 0 : n))}
              ><Icon name="star" /></button
            >
          {/each}
        </span>
        <label
          class="advance"
          title="Go to the next photo after every decision (or hold Shift: Shift+P, Shift+X)"
        >
          <input
            type="checkbox"
            bind:checked={autoAdvance}
            onchange={(e) => e.currentTarget.blur()}
            data-testid="loupe-auto-advance"
          />
          Auto-advance
        </label>
      </div>
    {/if}
  </div>

  {#if showInfo}
    <aside class="info" data-testid="loupe-info">
      {#if detail}
        <h2>{photo?.file_name}</h2>
        <dl>
          {#if detail.captured_at}
            <dt>Taken</dt>
            <dd>
              {detail.captured_at.replace("T", " ")}{detail.utc_offset
                ? ` (UTC${detail.utc_offset})`
                : ""}
            </dd>
          {/if}
          {#if detail.camera}<dt>Camera</dt>
            <dd>{detail.camera}</dd>{/if}
          {#if detail.lens}<dt>Lens</dt>
            <dd>{detail.lens}</dd>{/if}
          {#if detail.aperture || detail.shutter || detail.iso || detail.focal_length}
            <dt>Exposure</dt>
            <dd data-testid="loupe-exposure">
              {[
                detail.aperture,
                detail.shutter,
                detail.iso ? `ISO ${detail.iso}` : null,
                detail.focal_length,
              ]
                .filter(Boolean)
                .join(" · ")}
            </dd>
          {/if}
          {#if detail.score !== null}
            <dt>Score</dt>
            <dd
              data-testid="loupe-score"
              title="Combined quality: technical, aesthetic and faces"
            >
              {percent(detail.score)}
            </dd>
          {/if}
          {#if detail.quality}
            <dt>Quality</dt>
            <dd
              data-testid="loupe-quality"
              title="Technical scores from background analysis"
            >
              Sharpness {percent(detail.quality.sharpness)} · Exposure {percent(
                detail.quality.exposure,
              )} · Low noise {percent(detail.quality.noise)}
            </dd>
          {/if}
          {#if detail.aesthetic}
            <dt>Aesthetic</dt>
            <dd
              data-testid="loupe-aesthetic"
              title={`Rated ${detail.aesthetic.rating.toFixed(1)} of 10 by the aesthetic model`}
            >
              {percent(detail.aesthetic.score)}
            </dd>
          {/if}
          {#if detail.taste !== null}
            <dt>Your taste</dt>
            <dd
              data-testid="loupe-taste"
              title="How much it looks like the photos you pick rather than the ones you reject"
            >
              {percent(detail.taste)}
            </dd>
          {/if}
          {#if detail.faces && detail.faces.length > 0}
            {@const closed = detail.faces.filter((f) => f.eyes_closed).length}
            <dt>Faces</dt>
            <dd data-testid="loupe-faces">
              {detail.faces.length}
              {detail.faces.length === 1 ? "face" : "faces"}{closed > 0
                ? ` · ${closed} with closed eyes`
                : ""} · sharpest {percent(
                Math.max(...detail.faces.map((f) => f.sharpness)),
              )}
            </dd>
          {/if}
          {#if detail.width && detail.height}<dt>Size</dt>
            <dd>{detail.width} × {detail.height}</dd>{/if}
          {#if detail.place}<dt>Place</dt>
            <dd>{detail.place}</dd>{/if}
          {#if detail.latitude !== null && detail.longitude !== null}
            <dt>GPS</dt>
            <dd>{detail.latitude.toFixed(5)}, {detail.longitude.toFixed(5)}</dd>
          {/if}
          <dt>Files</dt>
          <dd>
            {#each detail.files as file (file.path)}
              <div class="file">
                <code>{file.path}</code>
                <span class="muted">{bytes(file.size)}</span>
                {#if libraryFolder && canReveal()}
                  {@const folder = libraryFolder}
                  <button
                    class="link"
                    onclick={() =>
                      void revealInFolder(joinPath(folder, file.path))}
                    title="Show this file in the file manager"
                    data-testid="reveal-file">Show in folder</button
                  >
                {/if}
              </div>
            {/each}
          </dd>
        </dl>
      {:else if detailError}
        <p class="error-text">{detailError}</p>
      {/if}
      {#if detail && detail.reasons.length > 0}
        <h3 class="why">Why this score</h3>
        <ul class="reasons" data-testid="loupe-reasons">
          {#each detail.reasons as reason (reason.text)}
            <li class={reason.tone}>
              <span class="mark" aria-hidden="true"
                >{#if reason.tone === "good"}<Icon
                    name="check"
                  />{:else if reason.tone === "bad"}<Icon
                    name="x-mark"
                  />{:else}•{/if}</span
              >{reason.text}
            </li>
          {/each}
        </ul>
      {/if}
      {#if photo}
        <Critique
          photoId={photo.id}
          status={critic}
          load={getCritique}
          run={critiquePhoto}
        />
      {/if}
      <p class="keys muted">
        ← → move · P or ↑ pick · X or ↓ reject · U unflag · 0–5 stars ·
        Shift+key decides and moves on · Z zoom · I info{photo && isPair(photo)
          ? " · R RAW or JPEG"
          : ""} · Esc close
      </p>
    </aside>
  {/if}
</div>

<style>
  .loupe {
    position: fixed;
    inset: 0;
    z-index: 10;
    display: flex;
    background: #0d0d0c;
    color: #ececea;
  }

  .stage {
    position: relative;
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .fitted {
    max-width: calc(100% - 2rem);
    max-height: calc(100% - 5rem);
    object-fit: contain;
    cursor: zoom-in;
    user-select: none;
  }

  .scroller {
    position: absolute;
    inset: 0;
    overflow: auto;
    cursor: grab;
    display: grid;
    place-items: center;
  }

  .scroller:active {
    cursor: grabbing;
  }

  .full {
    max-width: none;
    user-select: none;
  }

  .fitted.gone {
    display: none;
  }

  .full.hidden {
    visibility: hidden;
    position: absolute;
  }

  .loading {
    color: #9a9a94;
  }

  .toolbar {
    position: absolute;
    bottom: 1rem;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.35rem 0.5rem;
    background: rgb(30 30 28 / 0.85);
    border-radius: 10px;
    font-size: 0.85rem;
  }

  .toolbar button {
    background: transparent;
    color: inherit;
    border-color: #3a3a36;
    padding: 0.25rem 0.6rem;
  }

  .toolbar button[aria-pressed="true"] {
    background: #3a3a36;
  }

  .pair {
    display: flex;
  }

  .pair button {
    font-size: 0.8rem;
    font-weight: 700;
  }

  .pair button:first-child {
    border-top-right-radius: 0;
    border-bottom-right-radius: 0;
  }

  .pair button:last-child {
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
    border-left: none;
  }

  .format {
    font-size: 0.8rem;
    font-weight: 700;
    color: #b5b5ae;
    padding: 0 0.3rem;
  }

  .toolbar.cull {
    top: 1rem;
    bottom: auto;
  }

  .cull .pick[aria-pressed="true"] {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-fg);
  }

  .cull .reject[aria-pressed="true"] {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }

  .cull .auto {
    color: #8ee3ad;
    border: 1.5px solid #8ee3ad;
    border-radius: 6px;
    padding: 0.15rem 0.45rem;
    font-size: 0.8rem;
    white-space: nowrap;
  }

  .cull .stars {
    display: flex;
  }

  .cull .star {
    border: none;
    padding: 0.2rem 0.2rem;
    color: #6b6b66;
    --icon-size: 1.15rem;
  }

  .cull .star:hover:not(.on) {
    color: #9a9a94;
  }

  .cull .star.on {
    color: #f2c94c;
  }

  .cull .star[aria-pressed="true"] {
    background: transparent;
  }

  .advance {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    margin-left: 0.4rem;
    white-space: nowrap;
  }

  .info {
    width: 20rem;
    flex-shrink: 0;
    padding: 1.2rem;
    background: #171716;
    border-left: 1px solid #2a2a27;
    overflow-y: auto;
    font-size: 0.85rem;
  }

  .info h2 {
    font-size: 1rem;
    overflow-wrap: anywhere;
    margin-bottom: 0.8rem;
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.45rem 0.8rem;
    margin: 0;
  }

  dt {
    color: #9a9a94;
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .file {
    margin-bottom: 0.2rem;
  }

  .why {
    margin: 1.2rem 0 0.4rem;
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: #9a9a94;
  }

  .reasons {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .reasons li {
    display: flex;
    gap: 0.5rem;
  }

  .mark {
    width: 1rem;
    flex-shrink: 0;
    display: inline-flex;
    justify-content: center;
    align-items: center;
    font-weight: 700;
    --icon-size: 0.95rem;
  }

  .reasons .good .mark {
    color: #3fb36b;
  }

  .reasons .bad .mark {
    color: #ef6a5b;
  }

  .reasons .info .mark {
    color: #9a9a94;
  }

  .keys {
    margin-top: 1.5rem;
    font-size: 0.8rem;
  }

  @media (max-width: 900px) {
    .info {
      display: none;
    }
  }
</style>
