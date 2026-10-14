import { SvelteSet } from "svelte/reactivity";
import type { CameraKey, PhotoQuery, PhotoSort, PhotoSummary } from "../api";

type Flag = PhotoSummary["flag"];

/** A place filter: a country, optionally narrowed to a city. */
export interface PlaceFilter {
  country_code: string;
  city?: string;
  label: string;
}

/** An active filter shown as a removable chip. */
export interface Chip {
  label: string;
  clear: () => void;
}

const STORAGE_KEY = "picturesque.library.filters";
const FLAG_LABELS: Record<Flag, string> = {
  pick: "Picks",
  unflagged: "Unflagged",
  reject: "Rejects",
};

/** A technical score that can be filtered on. */
export type Score = "sharpness" | "exposure" | "noise" | "aesthetic";

export const SCORE_LABELS: Record<Score, string> = {
  sharpness: "Sharpness",
  exposure: "Exposure",
  noise: "Low noise",
  aesthetic: "Aesthetic",
};

interface Saved {
  from: string;
  to: string;
  flags: Flag[];
  autoPicked: boolean;
  minRating: number;
  camera: CameraKey | null;
  lens: string | null;
  place: PlaceFilter | null;
  minScores: Record<Score, number>;
  bestOnly: boolean;
  hideClosedEyes: boolean;
  people: boolean | null;
  search: string;
  stacked: boolean;
  sort: PhotoSort;
}

const NO_SCORES: Record<Score, number> = {
  sharpness: 0,
  exposure: 0,
  noise: 0,
  aesthetic: 0,
};

/** The library's filters. Kept for the session (sessionStorage). */
export class Filters {
  from = $state("");
  to = $state("");
  flags = $state<Flag[]>([]);
  /** Auto-picked photos; with flags, photos matching either. */
  autoPicked = $state(false);
  minRating = $state(0);
  camera = $state<CameraKey | null>(null);
  cameraLabel = $state("");
  lens = $state<string | null>(null);
  place = $state<PlaceFilter | null>(null);
  /** Minimum technical scores, 0–1 (0: any). */
  minScores = $state<Record<Score, number>>({ ...NO_SCORES });
  /** Hide series members other than the best photo. */
  bestOnly = $state(false);
  /** Hide photos where someone has closed eyes. */
  hideClosedEyes = $state(false);
  /** Only photos with (`true`) or without (`false`) people. */
  people = $state<boolean | null>(null);
  /** A description to search for; results replace the timeline. */
  search = $state("");
  /** Timeline order. Not a filter: kept by `clear`. */
  sort = $state<PhotoSort>("captured_desc");
  /** Show each series as its best photo. Not a filter: kept by `clear`. */
  stacked = $state(false);
  /** Stacked series shown in full; not saved. */
  readonly expanded = new SvelteSet<number>();

  constructor(
    private readonly storage: Pick<
      Storage,
      "getItem" | "setItem"
    > | null = sessionStorageOrNull(),
  ) {
    try {
      const saved = JSON.parse(
        this.storage?.getItem(STORAGE_KEY) ?? "null",
      ) as Partial<Saved> | null;
      if (saved) {
        this.from = saved.from ?? "";
        this.to = saved.to ?? "";
        this.flags = saved.flags ?? [];
        this.autoPicked = saved.autoPicked ?? false;
        this.minRating = saved.minRating ?? 0;
        this.camera = saved.camera ?? null;
        this.lens = saved.lens ?? null;
        this.place = saved.place ?? null;
        this.minScores = { ...NO_SCORES, ...saved.minScores };
        this.bestOnly = saved.bestOnly ?? false;
        this.search = saved.search ?? "";
        this.hideClosedEyes = saved.hideClosedEyes ?? false;
        this.people = saved.people ?? null;
        this.stacked = saved.stacked ?? false;
        this.sort = saved.sort ?? "captured_desc";
      }
    } catch {
      // Unreadable saved filters: start without filters.
    }
  }

  /** The filters as a `query_photos` query (without paging). */
  toQuery(): PhotoQuery {
    const q: PhotoQuery = {};
    if (this.from) q.from = this.from;
    if (this.to) q.to = this.to;
    if (this.flags.length > 0) q.flags = [...this.flags];
    if (this.autoPicked) q.auto_picked = true;
    if (this.minRating > 0) q.min_rating = this.minRating;
    if (this.camera) q.camera = { ...this.camera };
    if (this.lens) q.lens = this.lens;
    if (this.place) {
      q.country_code = this.place.country_code;
      if (this.place.city) q.city = this.place.city;
    }
    if (this.minScores.sharpness > 0)
      q.min_sharpness = this.minScores.sharpness;
    if (this.minScores.exposure > 0) q.min_exposure = this.minScores.exposure;
    if (this.minScores.noise > 0) q.min_noise = this.minScores.noise;
    if (this.minScores.aesthetic > 0)
      q.min_aesthetic = this.minScores.aesthetic;
    if (this.hideClosedEyes) q.hide_closed_eyes = true;
    if (this.people !== null) q.people = this.people;
    if (this.bestOnly) q.best_only = true;
    if (this.sort !== "captured_desc") q.sort = this.sort;
    if (this.stacked) {
      q.collapse_series = true;
      if (this.expanded.size > 0)
        q.expanded_series = [...this.expanded].sort((a, b) => a - b);
    }
    return q;
  }

  get active(): boolean {
    return this.chips.length > 0;
  }

  get chips(): Chip[] {
    const chips: Chip[] = [];
    if (this.from || this.to) {
      const label =
        this.from && this.to
          ? `${this.from} – ${this.to}`
          : this.from
            ? `From ${this.from}`
            : `Until ${this.to}`;
      chips.push({ label, clear: () => ((this.from = ""), (this.to = "")) });
    }
    for (const flag of this.flags) {
      chips.push({
        label: FLAG_LABELS[flag],
        clear: () => this.toggleFlag(flag),
      });
    }
    if (this.autoPicked)
      chips.push({
        label: "Auto-picked",
        clear: () => (this.autoPicked = false),
      });
    if (this.minRating > 0)
      chips.push({
        label: `${"★".repeat(this.minRating)} or more`,
        clear: () => (this.minRating = 0),
      });
    if (this.camera)
      chips.push({
        label: this.cameraLabel || this.camera.model,
        clear: () => (this.camera = null),
      });
    if (this.lens)
      chips.push({ label: this.lens, clear: () => (this.lens = null) });
    if (this.place)
      chips.push({ label: this.place.label, clear: () => (this.place = null) });
    for (const score of Object.keys(SCORE_LABELS) as Score[]) {
      const min = this.minScores[score];
      if (min > 0)
        chips.push({
          label: `${SCORE_LABELS[score]} ≥ ${Math.round(min * 100)}%`,
          clear: () => (this.minScores = { ...this.minScores, [score]: 0 }),
        });
    }
    if (this.bestOnly)
      chips.push({ label: "Best only", clear: () => (this.bestOnly = false) });
    if (this.hideClosedEyes)
      chips.push({
        label: "No closed eyes",
        clear: () => (this.hideClosedEyes = false),
      });
    if (this.people !== null)
      chips.push({
        label: this.people ? "With people" : "Without people",
        clear: () => (this.people = null),
      });
    if (this.search)
      chips.push({
        label: `“${this.search}”`,
        clear: () => (this.search = ""),
      });
    return chips;
  }

  toggleFlag(flag: Flag): void {
    this.flags = this.flags.includes(flag)
      ? this.flags.filter((f) => f !== flag)
      : [...this.flags, flag];
  }

  /** Expands a stacked series, or stacks it again. */
  toggleExpanded(seriesId: number): void {
    if (!this.expanded.delete(seriesId)) this.expanded.add(seriesId);
  }

  clear(): void {
    this.from = "";
    this.to = "";
    this.flags = [];
    this.autoPicked = false;
    this.minRating = 0;
    this.camera = null;
    this.lens = null;
    this.place = null;
    this.minScores = { ...NO_SCORES };
    this.bestOnly = false;
    this.hideClosedEyes = false;
    this.people = null;
    this.search = "";
  }

  /** Saves the filters for the rest of the session. */
  save(): void {
    const saved: Saved = {
      from: this.from,
      to: this.to,
      flags: this.flags,
      autoPicked: this.autoPicked,
      minRating: this.minRating,
      camera: this.camera,
      lens: this.lens,
      place: this.place,
      minScores: this.minScores,
      bestOnly: this.bestOnly,
      hideClosedEyes: this.hideClosedEyes,
      people: this.people,
      search: this.search,
      stacked: this.stacked,
      sort: this.sort,
    };
    try {
      this.storage?.setItem(STORAGE_KEY, JSON.stringify(saved));
    } catch {
      // Storage may be unavailable (private mode); filters still work.
    }
  }
}

function sessionStorageOrNull(): Storage | null {
  try {
    return typeof sessionStorage === "undefined" ? null : sessionStorage;
  } catch {
    return null;
  }
}
