import { expect, test } from "@playwright/test";
import { jpegWithGear, type Gear } from "./fixtures";
import { call, importFiles } from "./helpers";

const DAY = "2004-07-08";
const ALPHA = { make: "Statcam", model: "Alpha", lens: "Stat 24-70" };
const BETA = { make: "Statcam", model: "Beta", lens: "Stat 85" };
const GAMMA = { make: "Statcam", model: "Gamma" };

interface Row {
  label: string;
  counts: { photos: number; picks: number; auto_picked: number };
  few_photos: boolean;
  focal_lengths: { value: number; count: number }[];
}

test("ranks cameras and lenses by keepers and lists focal lengths", async ({
  page,
  request,
}) => {
  const files: Record<string, { bytes: Buffer; time: Date }> = {};
  const add = (name: string, n: number, gear: Gear, seed: number) => {
    for (let i = 0; i < n; i++)
      files[`stats/${name}_${i}.JPG`] = {
        bytes: jpegWithGear(seed + i, gear),
        time: new Date(`${DAY}T10:${String(seed % 60).padStart(2, "0")}:00`),
      };
  };
  add("alpha35", 20, { ...ALPHA, focal: 35 }, 12000);
  add("alpha50", 5, { ...ALPHA, focal: 50 }, 12100);
  add("beta", 22, { ...BETA, focal: 85 }, 12200);
  add("gamma", 3, GAMMA, 12300);
  await importFiles(request, files);

  const photos = (
    await call<{ photos: { id: number; camera: string | null }[] }>(
      request,
      "query_photos",
      { from: DAY, to: DAY, limit: 1000 },
    )
  ).photos;
  const of = (model: string) =>
    photos.filter((p) => p.camera?.endsWith(model)).map((p) => p.id);
  const flag = (ids: number[], f: string) =>
    call(request, "set_decisions", {
      changes: ids.map((photo_id) => ({ photo_id, flag: f })),
    });
  await flag(of("Alpha").slice(0, 10), "pick");
  await flag(of("Beta").slice(0, 2), "pick");
  await flag(of("Beta").slice(2, 7), "reject");

  const stats = await call<{ cameras: Row[]; lenses: Row[] }>(
    request,
    "library_stats",
    null,
  );
  const camera = (label: string) =>
    stats.cameras.find((c) => c.label === label)!;
  const alpha = camera("Statcam Alpha");
  expect(alpha.counts).toMatchObject({ photos: 25, picks: 10 });
  expect(alpha.focal_lengths).toEqual([
    { value: 35, count: 20 },
    { value: 50, count: 5 },
  ]);
  expect(camera("Statcam Beta").counts).toMatchObject({
    photos: 22,
    picks: 2,
    rejects: 5,
  });
  expect(camera("Statcam Gamma").few_photos).toBe(true);

  await page.goto("/");
  await page
    .getByRole("navigation", { name: "Main" })
    .getByRole("button", { name: "Statistics" })
    .click();

  // The table follows the API's ranking; few-photo gear comes last.
  const rows = page.getByTestId("camera-stats").getByTestId("gear-row");
  const labels = (await rows.locator("th").allTextContents()).filter((l) =>
    l.startsWith("Statcam"),
  );
  expect(labels).toEqual(
    stats.cameras.map((c) => c.label).filter((l) => l.startsWith("Statcam")),
  );
  expect(labels.at(-1)).toBe("Statcam Gamma");
  const keepers = alpha.counts.picks + alpha.counts.auto_picked;
  await expect(
    rows.filter({ hasText: "Statcam Alpha" }).getByTestId("keeper-rate"),
  ).toHaveText(`${Math.round((keepers / 25) * 100)}%`);
  await expect(
    page.getByTestId("lens-stats").getByRole("rowheader", { name: "Stat 85" }),
  ).toBeVisible();

  const focal = page
    .getByTestId("focal-lengths")
    .filter({ hasText: "Statcam Alpha" });
  await expect(focal.locator("li").first()).toContainText("35 mm");
  await expect(focal.locator("li").first()).toContainText("80%");

  // Decisions made elsewhere show when the page is opened again.
  await flag(of("Alpha").slice(10, 25), "pick");
  await page
    .getByRole("navigation", { name: "Main" })
    .getByRole("button", { name: "Library" })
    .click();
  await page
    .getByRole("navigation", { name: "Main" })
    .getByRole("button", { name: "Statistics" })
    .click();
  await expect(
    rows.filter({ hasText: "Statcam Alpha" }).getByTestId("keeper-rate"),
  ).toHaveText("100%");
});
