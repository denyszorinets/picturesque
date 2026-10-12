import type { CameraKey, PhotoQuery, PhotoSummary } from "../api";

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

interface Saved {
  from: string;
  to: string;
  flags: Flag[];
  minRating: number;
  camera: CameraKey | null;
  lens: string | null;
  place: PlaceFilter | null;
}

/** The library's filters. Kept for the session (sessionStorage). */
export class Filters {
  from = $state("");
  to = $state("");
  flags = $state<Flag[]>([]);
  minRating = $state(0);
  camera = $state<CameraKey | null>(null);
  cameraLabel = $state("");
  lens = $state<string | null>(null);
  place = $state<PlaceFilter | null>(null);

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
        this.minRating = saved.minRating ?? 0;
        this.camera = saved.camera ?? null;
        this.lens = saved.lens ?? null;
        this.place = saved.place ?? null;
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
    if (this.minRating > 0) q.min_rating = this.minRating;
    if (this.camera) q.camera = { ...this.camera };
    if (this.lens) q.lens = this.lens;
    if (this.place) {
      q.country_code = this.place.country_code;
      if (this.place.city) q.city = this.place.city;
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
    return chips;
  }

  toggleFlag(flag: Flag): void {
    this.flags = this.flags.includes(flag)
      ? this.flags.filter((f) => f !== flag)
      : [...this.flags, flag];
  }

  clear(): void {
    this.from = "";
    this.to = "";
    this.flags = [];
    this.minRating = 0;
    this.camera = null;
    this.lens = null;
    this.place = null;
  }

  /** Saves the filters for the rest of the session. */
  save(): void {
    const saved: Saved = {
      from: this.from,
      to: this.to,
      flags: this.flags,
      minRating: this.minRating,
      camera: this.camera,
      lens: this.lens,
      place: this.place,
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
