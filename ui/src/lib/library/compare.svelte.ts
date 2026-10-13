/** How many photos the compare view shows side by side. */
export type Panes = 2 | 4;

/** A point in a photo, 0–1 from the top left. */
export interface Point {
  x: number;
  y: number;
}

const clamp = (v: number, lo: number, hi: number) =>
  Math.min(hi, Math.max(lo, v));

/**
 * The compare view's state: the photos being compared, which of them are
 * on screen (a window of `panes` photos that follows the active one), and
 * the zoom shared by every pane.
 */
export class CompareState {
  /** Index of the active photo in `ids`. */
  active = $state(0);
  panes = $state<Panes>(2);
  /** Index of the first photo on screen. */
  start = $state(0);
  /** Whether panes show photos at 100%. */
  zoomed = $state(false);
  /** The point of the photos at the centre of every zoomed pane. */
  center = $state<Point>({ x: 0.5, y: 0.5 });

  constructor(
    readonly ids: readonly number[],
    activeId?: number,
    panes: Panes = ids.length > 2 ? 4 : 2,
  ) {
    this.panes = panes;
    this.moveTo(
      Math.max(0, activeId === undefined ? 0 : ids.indexOf(activeId)),
    );
  }

  get activeId(): number | undefined {
    return this.ids[this.active];
  }

  /** Indices of the photos on screen, in order. */
  get visible(): number[] {
    const end = Math.min(this.ids.length, this.start + this.panes);
    return Array.from({ length: end - this.start }, (_, k) => this.start + k);
  }

  moveTo(index: number): void {
    if (this.ids.length === 0) return;
    this.active = clamp(index, 0, this.ids.length - 1);
    this.follow();
  }

  move(delta: number): void {
    this.moveTo(this.active + delta);
  }

  togglePanes(): void {
    this.panes = this.panes === 2 ? 4 : 2;
    this.follow();
  }

  /** Zooms to 100% around `at`, or back to fitted. */
  toggleZoom(at: Point = { x: 0.5, y: 0.5 }): void {
    if (!this.zoomed)
      this.center = { x: clamp(at.x, 0, 1), y: clamp(at.y, 0, 1) };
    this.zoomed = !this.zoomed;
  }

  /** Moves the zoomed view by a fraction of the photo. */
  pan(dx: number, dy: number): void {
    this.center = {
      x: clamp(this.center.x + dx, 0, 1),
      y: clamp(this.center.y + dy, 0, 1),
    };
  }

  /** Scrolls the window so the active photo is on screen, filling it. */
  private follow(): void {
    let start = this.start;
    if (this.active < start) start = this.active;
    if (this.active >= start + this.panes) start = this.active - this.panes + 1;
    this.start = clamp(start, 0, Math.max(0, this.ids.length - this.panes));
  }
}
