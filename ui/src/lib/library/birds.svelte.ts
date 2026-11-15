import { SvelteMap } from "svelte/reactivity";
import type { PhotoSpeciesView, SpeciesView } from "../api";

/** What the names need from the backend (injectable for tests). */
export interface BirdNamesApi {
  photoSpecies(photoIds: number[]): Promise<PhotoSpeciesView[]>;
  setPhotoSpecies(photoId: number, species: SpeciesView[]): Promise<void>;
}

const NONE: SpeciesView[] = [];

/**
 * The species chosen for photos (Bird ID): several per photo, in the
 * order chosen. Loaded as photos come into view and kept current as
 * species are added or removed.
 */
export class BirdNames {
  /** Photo id → species (empty once known to have none). */
  private readonly known = new SvelteMap<number, SpeciesView[]>();

  constructor(private readonly api: BirdNamesApi) {}

  /** The species of a photo (empty if none, or not loaded yet). */
  of(photoId: number): SpeciesView[] {
    return this.known.get(photoId) ?? NONE;
  }

  /** Loads the species of the photos not asked about yet. */
  async load(photoIds: number[]): Promise<void> {
    const missing = photoIds.filter(
      (id, i) => !this.known.has(id) && photoIds.indexOf(id) === i,
    );
    if (missing.length === 0) return;
    // Marked first so concurrent loads do not ask twice.
    for (const id of missing) this.known.set(id, NONE);
    try {
      for (const { photo_id, species } of await this.api.photoSpecies(missing))
        this.known.set(photo_id, species);
    } catch {
      for (const id of missing) this.known.delete(id);
    }
  }

  /** Adds a species to each of the photos (once). */
  async add(photoIds: number[], species: SpeciesView): Promise<void> {
    await this.load(photoIds);
    for (const id of photoIds) {
      const now = this.of(id);
      if (now.some((s) => s.code === species.code)) continue;
      const next = [...now, species];
      await this.api.setPhotoSpecies(id, next);
      this.known.set(id, next);
    }
  }

  /** Removes a species from each of the photos. */
  async remove(photoIds: number[], code: string): Promise<void> {
    await this.load(photoIds);
    for (const id of photoIds) {
      const now = this.of(id);
      if (!now.some((s) => s.code === code)) continue;
      const next = now.filter((s) => s.code !== code);
      await this.api.setPhotoSpecies(id, next);
      this.known.set(id, next);
    }
  }
}

/** Common names, joined for a label ("Mallard, Wood Duck"). */
export function names(species: SpeciesView[]): string | null {
  return species.length === 0 ? null : species.map((s) => s.common).join(", ");
}
