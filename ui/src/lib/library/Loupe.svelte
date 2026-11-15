<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import Critique from "./Critique.svelte";
  import SeriesRibbon from "./SeriesRibbon.svelte";
  import UpcomingStacks from "./UpcomingStacks.svelte";
  import { untrack } from "svelte";
  import BirdIdPanel, { type Bird } from "./BirdIdPanel.svelte";
  import { BirdNames, names } from "./birds.svelte";
  import ResizeHandle from "../components/ResizeHandle.svelte";
  import { SeriesStrip, stripPrefs } from "./series-strip.svelte";
  import { entryMember, step, type Entry } from "./stack-nav";
  import {
    ApiError,
    canReveal,
    criticStatus,
    critiquePhoto,
    getCritique,
    getPhoto,
    findBirds,
    identifyBird,
    photoSpecies,
    setPhotoSpecies,
    type CriticStatus,
    type DecisionView,
    type SpeciesView,
    revealInFolder,
    type PairView,
    type PhotoDetail,
    type PhotoSummary,
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
    /** Whether series are stacked: the loupe then steps through each
     * stack's photos before moving on, and shows the stacks up next. */
    stacked?: boolean;
    /** The photo shown: a member of the open stack, or the list entry. */
    shown?: PhotoSummary | null;
    /** Follows decisions (so the series ribbon stays current). */
    subscribeDecisions?: (
      listener: (decisions: DecisionView[]) => void,
    ) => () => void;
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
    stacked = false,
    // Write-only here: the parent reads it through bind:shown.
    // eslint-disable-next-line no-useless-assignment
    shown = $bindable(null),
    subscribeDecisions,
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

  /** The list entry: with stacked series, the photo shown for the stack. */
  const listPhoto = $derived(store.photos[index]);
  /** With stacked series, the member of the open stack shown instead of
   * the list entry; `null` shows the entry itself. */
  let memberId = $state<number | null>(null);
  /** Where to enter the stack just opened, once its members are known. */
  let pendingEntry = $state<Entry | null>(null);

  // Another entry starts at its own photo (unless entered at an end).
  $effect(() => {
    void index;
    memberId = null;
  });

  // The entry's series: the ribbon, stepping through stacks, and what to
  // compare with.
  const strip = new SeriesStrip();
  $effect(() => {
    // Decisions change what the ribbon shows (rejects, picks, the best).
    void [
      listPhoto?.id,
      listPhoto?.flag,
      listPhoto?.best_in_series,
      listPhoto?.series_id,
    ];
    void strip.load(listPhoto);
  });
  $effect(() => subscribeDecisions?.((d) => strip.applyDecisions(d)));
  /** The stack's members are loaded for the entry shown. */
  const stripReady = $derived(!!listPhoto && strip.loadedFor === listPhoto.id);
  $effect(() => {
    const entry = pendingEntry;
    if (entry === null || !stripReady) return;
    pendingEntry = null;
    memberId = entryMember(
      strip.members.map((p) => p.id),
      entry,
    );
  });
  /** Entering a stack at one end: wait for its members rather than
   * flashing the stack's best photo first. */
  const entering = $derived(
    stacked &&
      pendingEntry !== null &&
      !!listPhoto &&
      listPhoto.series_size > 1 &&
      !stripReady,
  );

  const photo = $derived(
    (memberId !== null && stripReady
      ? strip.members.find((p) => p.id === memberId)
      : undefined) ?? listPhoto,
  );
  $effect(() => {
    shown = photo ?? null;
  });

  const inSeries = $derived(!!photo && strip.position(photo.id) !== null);
  const rival = $derived(
    inSeries && stripPrefs.compareBest && photo ? strip.rival(photo.id) : null,
  );
  const inList = (id: number) => store.photos.some((p) => p.id === id);
  /** Ribbon frames that can be opened: with stacked series, every member
   * of the open stack; otherwise those in the list. */
  const reachable = (id: number) =>
    (stacked && strip.members.some((p) => p.id === id)) || inList(id);
  function openFromRibbon(id: number) {
    if (stacked && strip.members.some((p) => p.id === id)) {
      memberId = id === listPhoto?.id ? null : id;
      return;
    }
    const at = store.photos.findIndex((p) => p.id === id);
    if (at >= 0) onnavigate(at);
  }

  /** Width of the details pane, resizable. */
  let infoWidth = $state(320);

  // ---- Bird ID ----
  const birdNames = new BirdNames({ photoSpecies, setPhotoSpecies });
  /** The Bird ID panel is open. */
  let birdOpen = $state(false);
  /** Waiting for a box to be drawn on the photo. */
  let drawing = $state(false);
  let birdBusy = $state(false);
  /** Birds found or boxed in the shown photo, and the one looked at. */
  let birds = $state<Bird[]>([]);
  let selectedBird = $state<number | null>(null);
  let birdRegion = $state<string | null>(null);
  let birdError = $state<string | null>(null);
  /** The box being drawn: shares of the shown photo. */
  let birdBox = $state<{ x: number; y: number; w: number; h: number } | null>(
    null,
  );
  let fittedImg: HTMLImageElement | undefined = $state();
  let stageEl: HTMLDivElement | undefined = $state();
  /** Where the fitted photo is in the stage, in pixels. */
  let imgBox = $state<{
    left: number;
    top: number;
    width: number;
    height: number;
  } | null>(null);
  let boxStart: { x: number; y: number } | null = null;

  function measure() {
    if (!fittedImg || !stageEl) return;
    const i = fittedImg.getBoundingClientRect();
    const s = stageEl.getBoundingClientRect();
    imgBox = {
      left: i.left - s.left,
      top: i.top - s.top,
      width: i.width,
      height: i.height,
    };
  }

  function openBird() {
    zoomed = false;
    birdOpen = true;
    birdError = null;
    // After the fitted image is back in place.
    requestAnimationFrame(measure);
  }

  function startDrawing() {
    openBird();
    drawing = true;
    birdBox = null;
  }

  /** Finds every bird in the shown photo. */
  async function findAll() {
    const shown = photo;
    if (!shown) return;
    openBird();
    drawing = false;
    birdBusy = true;
    try {
      const found = await findBirds(shown.id);
      if (photo?.id !== shown.id) return;
      // Boxes drawn by hand stay.
      birds = [
        ...found.birds.map((b) => ({ ...b, score: b.score as number | null })),
        ...birds.filter((b) => b.score === null),
      ];
      birdRegion = found.region;
      selectedBird = birds.length > 0 ? 0 : null;
      if (found.birds.length === 0)
        birdError = "No bird found. Draw a box around one with D.";
    } catch (e) {
      if (photo?.id === shown.id) birdError = ApiError.from(e).message;
    } finally {
      birdBusy = false;
    }
  }

  function toggleBird() {
    if (birdOpen && drawing) {
      drawing = false;
      if (birds.length === 0) birdOpen = false;
    } else startDrawing();
  }

  // Another photo: what was found was about the previous one.
  $effect(() => {
    void photo?.id;
    birds = [];
    selectedBird = null;
    birdError = null;
    birdBox = null;
    drawing = false;
    // Only on a new photo: opening the panel must not reset it.
    if (untrack(() => birdOpen)) requestAnimationFrame(measure);
  });

  /** Position (shares of the photo, 0–1) of a pointer event. */
  function shareOf(e: PointerEvent) {
    if (!imgBox || !stageEl) return { x: 0, y: 0 };
    const s = stageEl.getBoundingClientRect();
    const clamp = (v: number) => Math.min(1, Math.max(0, v));
    return {
      x: clamp((e.clientX - s.left - imgBox.left) / imgBox.width),
      y: clamp((e.clientY - s.top - imgBox.top) / imgBox.height),
    };
  }

  function boxDown(e: PointerEvent) {
    boxStart = shareOf(e);
    birdBox = { ...boxStart, w: 0, h: 0 };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function boxMove(e: PointerEvent) {
    if (!boxStart) return;
    const p = shareOf(e);
    birdBox = {
      x: Math.min(boxStart.x, p.x),
      y: Math.min(boxStart.y, p.y),
      w: Math.abs(p.x - boxStart.x),
      h: Math.abs(p.y - boxStart.y),
    };
  }

  async function boxUp() {
    boxStart = null;
    const box = birdBox;
    const shown = photo;
    // A click, not a box.
    if (!box || box.w < 0.01 || box.h < 0.01 || !shown) {
      birdBox = null;
      return;
    }
    drawing = false;
    birdBox = null;
    birdBusy = true;
    birdError = null;
    const area = { x: box.x, y: box.y, width: box.w, height: box.h };
    try {
      const result = await identifyBird(shown.id, area);
      if (photo?.id !== shown.id) return;
      birds = [
        ...birds,
        {
          box: area,
          score: null,
          suggestions: result.suggestions,
          elsewhere: result.elsewhere,
        },
      ];
      birdRegion = result.region;
      selectedBird = birds.length - 1;
    } catch (e) {
      if (photo?.id === shown.id) birdError = ApiError.from(e).message;
    } finally {
      birdBusy = false;
    }
  }

  /** The photos a species choice applies to. */
  function namedIds(wholeStack: boolean): number[] {
    if (!photo) return [];
    return wholeStack ? strip.members.map((p) => p.id) : [photo.id];
  }

  async function addSpecies(species: SpeciesView, wholeStack: boolean) {
    try {
      await birdNames.add(namedIds(wholeStack), species);
    } catch (e) {
      birdError = ApiError.from(e).message;
    }
  }

  async function removeSpecies(code: string, wholeStack: boolean) {
    try {
      await birdNames.remove(namedIds(wholeStack), code);
    } catch (e) {
      birdError = ApiError.from(e).message;
    }
  }

  // Names of the shown photo, its stack and the stacks up next.
  $effect(() => {
    const ids = strip.members.map((p) => p.id);
    if (photo) ids.push(photo.id);
    if (stacked)
      for (
        let i = Math.max(0, index - 30);
        i < Math.min(store.total, index + 31);
        i++
      ) {
        const p = store.photos[i];
        if (p) ids.push(p.id);
      }
    void birdNames.load(ids);
  });
  const species = $derived(photo ? birdNames.of(photo.id) : []);

  const UPNEXT_KEY = "picturesque.upNext";
  /** The stacks-up-next pane, shown while stacked. */
  let showUpNext = $state(
    (() => {
      try {
        return localStorage.getItem(UPNEXT_KEY) !== "false";
      } catch {
        return true;
      }
    })(),
  );
  $effect(() => {
    try {
      localStorage.setItem(UPNEXT_KEY, String(showUpNext));
    } catch {
      // A convenience only.
    }
  });
  function toggleCompare() {
    if (inSeries) stripPrefs.compareBest = !stripPrefs.compareBest;
  }
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
    void photo?.id;
    zoomed = false;
    fullLoaded = false;
  });

  $effect(() => {
    void [photo?.id, view];
    broken = false;
  });

  // Preload neighbours so moving through photos feels instant.
  $effect(() => {
    const near: PhotoSummary[] = [];
    for (const i of [index - 1, index + 1]) {
      const neighbour = store.photos[i];
      if (neighbour) near.push(neighbour);
    }
    if (stacked && photo) {
      const at = strip.members.findIndex((p) => p.id === photo.id);
      for (const k of [at - 1, at + 1]) {
        const member = at >= 0 ? strip.members[k] : undefined;
        if (member) near.push(member);
      }
    }
    for (const p of near) new Image().src = photoUrl(p, "preview", view);
  });

  const percent = (score: number) => `${Math.round(score * 100)}%`;

  /** Where a step goes: through the open stack's photos when stacked. */
  function target(delta: 1 | -1, wholeStack = false) {
    return step({
      index,
      total: store.total,
      members: stacked && stripReady ? strip.members.map((p) => p.id) : [],
      currentId: photo?.id ?? null,
      delta,
      wholeStack,
    });
  }

  function go(delta: 1 | -1, wholeStack = false) {
    if (entering) return;
    const to = target(delta, wholeStack);
    if (!to) return;
    if (to.kind === "member") {
      memberId = to.id === listPhoto?.id ? null : to.id;
      return;
    }
    pendingEntry = stacked ? to.entry : null;
    onnavigate(to.index);
  }

  /** The next photo (also after a decision with Auto-advance). */
  export function next() {
    go(1);
  }

  /** Opens another list entry (from the stacks-up-next pane). */
  function openEntry(at: number) {
    pendingEntry = null;
    onnavigate(at);
  }

  const canBack = $derived(!entering && target(-1) !== null);
  const canForward = $derived(!entering && target(1) !== null);

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
      // Shift or Page Up/Down skips the rest of a stack.
      ArrowRight: () => go(1, e.shiftKey),
      ArrowLeft: () => go(-1, e.shiftKey),
      PageDown: () => go(1, true),
      PageUp: () => go(-1, true),
      n: () => (showUpNext = !showUpNext),
      N: () => (showUpNext = !showUpNext),
      d: toggleBird,
      D: toggleBird,
      f: () => void findAll(),
      F: () => void findAll(),
      Escape: () => {
        if (drawing) {
          drawing = false;
          birdBox = null;
          if (birds.length === 0) birdOpen = false;
        } else if (zoomed) zoomed = false;
        else if (birdOpen) birdOpen = false;
        else onclose();
      },
      z: () => toggleZoom(),
      Z: () => toggleZoom(),
      i: () => (showInfo = !showInfo),
      I: () => (showInfo = !showInfo),
      r: toggleView,
      R: toggleView,
      v: toggleCompare,
      V: toggleCompare,
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
  {#if stacked && showUpNext}
    <UpcomingStacks
      {store}
      {index}
      onselect={openEntry}
      species={(p) => names(birdNames.of(p.id))}
    />
  {/if}
  <div
    class="stage"
    class:has-ribbon={inSeries}
    class:versus={!!rival && !zoomed}
    bind:this={stageEl}
  >
    {#if entering}
      <p class="loading">Loading…</p>
    {:else if photo}
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
            onload={() => birdOpen && measure()}
            bind:this={fittedImg}
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

    {#if rival && !zoomed}
      <figure class="rival" data-testid="loupe-rival">
        {#key rival.photo.id}
          <img
            class="fitted"
            src={photoUrl(rival.photo, "preview", view)}
            alt={rival.photo.file_name}
            draggable="false"
            data-testid="loupe-rival-image"
          />
        {/key}
        <figcaption data-testid="loupe-rival-label">
          {rival.runnerUp ? "Runner-up" : "Best of the series"} · {rival.photo
            .file_name}
        </figcaption>
      </figure>
    {/if}

    {#if birdOpen && !zoomed && imgBox && (drawing || birdBox || birds.length > 0)}
      <div
        class="boxer"
        class:drawing
        style:left="{imgBox.left}px"
        style:top="{imgBox.top}px"
        style:width="{imgBox.width}px"
        style:height="{imgBox.height}px"
        role="presentation"
        onpointerdown={drawing ? boxDown : undefined}
        onpointermove={drawing ? boxMove : undefined}
        onpointerup={drawing ? () => void boxUp() : undefined}
        data-testid="bird-id-boxer"
      >
        {#each birds as bird, i (i)}
          <button
            class="birdbox found"
            class:selected={selectedBird === i}
            style:left="{bird.box.x * 100}%"
            style:top="{bird.box.y * 100}%"
            style:width="{bird.box.width * 100}%"
            style:height="{bird.box.height * 100}%"
            onclick={(e) => {
              e.currentTarget.blur();
              selectedBird = i;
            }}
            title={bird.suggestions[0]?.species.common ?? "Bird"}
            data-testid="bird-id-box"><span class="tag">{i + 1}</span></button
          >
        {/each}
        {#if birdBox}
          <div
            class="birdbox"
            style:left="{birdBox.x * 100}%"
            style:top="{birdBox.y * 100}%"
            style:width="{birdBox.w * 100}%"
            style:height="{birdBox.h * 100}%"
          ></div>
        {/if}
      </div>
    {/if}

    {#if birdOpen}
      <BirdIdPanel
        {birds}
        selected={selectedBird}
        onselect={(i) => (selectedBird = selectedBird === i ? null : i)}
        {drawing}
        busy={birdBusy}
        error={birdError}
        region={birdRegion}
        current={species}
        stackSize={strip.members.length}
        onadd={(s, whole) => void addSpecies(s, whole)}
        onremove={(code, whole) => void removeSpecies(code, whole)}
        onfind={() => void findAll()}
        ondraw={startDrawing}
        onclose={() => {
          birdOpen = false;
          drawing = false;
        }}
      />
    {/if}

    {#if inSeries && photo}
      <SeriesRibbon
        members={strip.members}
        currentId={photo.id}
        bestId={strip.bestId}
        {reachable}
        onselect={openFromRibbon}
      />
    {/if}

    <div class="toolbar">
      <button
        onclick={(e) => act(e, () => go(-1))}
        disabled={!canBack}
        aria-label="Previous photo"><Icon name="chevron-left" /></button
      >
      <span
        data-testid="loupe-position"
        title={stacked ? "Stack, of the stacks in view" : undefined}
        >{stacked ? "Stack " : ""}{index + 1} / {store.total}</span
      >
      <button
        onclick={(e) => act(e, () => go(1))}
        disabled={!canForward}
        aria-label="Next photo"><Icon name="chevron-right" /></button
      >
      {#if stacked}
        <button
          onclick={(e) => act(e, () => (showUpNext = !showUpNext))}
          aria-pressed={showUpNext}
          title="Show the stacks up next (N)"
          data-testid="loupe-upnext"><Icon name="queue-list" /> Up next</button
        >
      {/if}
      <button onclick={() => toggleZoom()} aria-pressed={zoomed}
        ><Icon name="magnifying-glass-plus" /> {zoomed ? "Fit" : "100%"}</button
      >
      <button onclick={() => (showInfo = !showInfo)} aria-pressed={showInfo}
        ><Icon name="information-circle" /> Info</button
      >
      <button
        onclick={(e) =>
          act(e, () => {
            if (birdOpen) {
              birdOpen = false;
              drawing = false;
            } else openBird();
          })}
        aria-pressed={birdOpen}
        title="Identify birds: find them all (F) or draw a box (D)"
        data-testid="loupe-bird-id"
        ><Icon name="viewfinder-circle" /> Bird ID</button
      >
      {#if inSeries}
        <button
          onclick={(e) => act(e, toggleCompare)}
          aria-pressed={stripPrefs.compareBest}
          title="Show the best photo of the series beside this one (V)"
          data-testid="loupe-compare-best"
          ><Icon name="squares-2x2" /> Compare with best</button
        >
      {/if}
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
    <aside class="info" style:width="{infoWidth}px" data-testid="loupe-info">
      <ResizeHandle
        bind:width={infoWidth}
        edge="left"
        min={240}
        max={640}
        storageKey="picturesque.infoWidth"
        label="Resize the details pane"
      />
      {#if detail}
        <h2>{photo?.file_name}</h2>
        <dl>
          {#if species.length > 0}
            <dt>{species.length === 1 ? "Bird" : "Birds"}</dt>
            <dd data-testid="loupe-species">
              {#each species as s (s.code)}
                <div>{s.common} <em class="muted">{s.scientific}</em></div>
              {/each}
            </dd>
          {/if}
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
          {#if detail.critique_score !== null}
            <dt>Critic</dt>
            <dd
              data-testid="loupe-critic"
              title="Where the critic ranks it among your critiqued photos"
            >
              {percent(detail.critique_score)}
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
        ← → move{stacked
          ? " through each stack · Shift+← → or Page Up/Down skip a stack · N up next"
          : ""} · P or ↑ pick · X or ↓ reject · U unflag · 0–5 stars · F find birds
        · D draw a box · Shift+key decides and moves on · Z zoom · I info{photo &&
        isPair(photo)
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
    --ribbon-height: 0px;
    position: relative;
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .stage.has-ribbon {
    --ribbon-height: 5.5rem;
  }

  .fitted {
    max-width: calc(100% - 2rem);
    max-height: calc(100% - 5rem - var(--ribbon-height));
    object-fit: contain;
    cursor: zoom-in;
    user-select: none;
  }

  /* Side by side: two equal columns between the toolbars and the ribbon. */
  .stage.versus {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    gap: 1rem;
    place-items: center;
    padding: 4.5rem 1rem calc(4.5rem + var(--ribbon-height));
    box-sizing: border-box;
  }

  .versus .fitted {
    max-width: 100%;
    max-height: 100%;
  }

  .rival {
    margin: 0;
    height: 100%;
    max-width: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    min-width: 0;
  }

  .versus .rival .fitted {
    max-height: calc(100% - 1.6rem);
    cursor: default;
  }

  .rival figcaption {
    color: #9a9a94;
    font-size: 0.8rem;
  }

  .boxer {
    position: absolute;
    z-index: 1;
  }

  .boxer.drawing {
    cursor: crosshair;
  }

  .birdbox {
    position: absolute;
    border: 2px solid #f2c94c;
    box-sizing: border-box;
    pointer-events: none;
  }

  /* The container lets clicks through to the photo unless drawing; the
     found birds' boxes take them. */
  .boxer:not(.drawing) {
    pointer-events: none;
  }

  .birdbox.found {
    padding: 0;
    background: transparent;
    border-radius: 2px;
    border-color: rgb(242 201 76 / 0.6);
    pointer-events: auto;
    cursor: pointer;
  }

  .boxer.drawing .birdbox.found {
    pointer-events: none;
  }

  .birdbox.found.selected {
    border-color: #f2c94c;
    border-width: 3px;
  }

  .tag {
    position: absolute;
    left: -2px;
    top: -1.4rem;
    min-width: 1.2rem;
    padding: 0 0.3rem;
    border-radius: 4px 4px 0 0;
    background: #f2c94c;
    color: #0d0d0c;
    font-size: 0.75rem;
    font-weight: 700;
    line-height: 1.4rem;
    text-align: center;
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
    bottom: calc(1rem + var(--ribbon-height));
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    /* Narrowed by the side panes, it wraps rather than overflowing. */
    flex-wrap: wrap;
    justify-content: center;
    align-items: center;
    gap: 0.4rem;
    padding: 0.35rem 0.5rem;
    width: max-content;
    max-width: calc(100% - 2rem);
    box-sizing: border-box;
    white-space: nowrap;
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
    position: relative;
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
