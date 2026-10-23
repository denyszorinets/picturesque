import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { call, importFiles } from "./helpers";

test("opens photos in the loupe and works from the keyboard", async ({
  page,
  request,
}) => {
  await importFiles(request, {
    "loupe/a.JPG": { bytes: jpeg(9001), time: new Date("2002-03-03T10:00:00") },
    "loupe/b.JPG": { bytes: jpeg(9002), time: new Date("2002-03-03T10:00:01") },
  });
  await page.goto("/");
  const tile = page.locator("[data-photo-id]").first();
  await tile.click();

  await page.keyboard.press("Enter");
  const loupe = page.getByTestId("loupe");
  await expect(loupe).toBeVisible();
  await expect(page.getByTestId("loupe-position")).toHaveText(/^1 \/ \d+$/);
  await expect(page.getByTestId("loupe-info")).toContainText("Files");
  const fitted = page.getByTestId("loupe-image");
  await expect
    .poll(() => fitted.evaluate((img: HTMLImageElement) => img.naturalWidth))
    .toBeGreaterThan(0);

  await page.keyboard.press("ArrowRight");
  await expect(page.getByTestId("loupe-position")).toHaveText(/^2 \/ \d+$/);
  await page.keyboard.press("ArrowLeft");
  await expect(page.getByTestId("loupe-position")).toHaveText(/^1 \/ \d+$/);

  await page.keyboard.press("i");
  await expect(page.getByTestId("loupe-info")).toBeHidden();

  // 100% zoom loads the full-size image.
  await page.keyboard.press("z");
  const full = page.getByTestId("loupe-zoomed").locator("img");
  await expect
    .poll(() => full.evaluate((img: HTMLImageElement) => img.naturalWidth))
    .toBeGreaterThan(0);
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("loupe-zoomed")).toBeHidden();

  await page.keyboard.press("Escape");
  await expect(loupe).toBeHidden();
  await expect(tile).toHaveAttribute("aria-pressed", "true");
});

test("culls in the loupe with buttons, keys and auto-advance", async ({
  page,
  request,
}) => {
  await importFiles(request, {
    "cull/a.JPG": { bytes: jpeg(9101), time: new Date("2003-04-04T10:00:00") },
    "cull/b.JPG": { bytes: jpeg(9102), time: new Date("2003-04-04T10:00:01") },
    "cull/c.JPG": { bytes: jpeg(9103), time: new Date("2003-04-04T10:00:02") },
  });
  await page.goto("/");
  await page.locator("[data-photo-id]").first().dblclick();
  const position = page.getByTestId("loupe-position");
  await expect(position).toHaveText(/^1 \/ \d+$/);
  const pick = page.getByTestId("loupe-pick");
  const reject = page.getByTestId("loupe-reject");

  // Buttons show and change the photo's decision.
  await pick.click();
  await expect(pick).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "3 stars" }).click();
  await expect(page.getByRole("button", { name: "3 stars" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  // Keys still work after clicking a button.
  await page.keyboard.press("x");
  await expect(reject).toHaveAttribute("aria-pressed", "true");
  await expect(position).toHaveText(/^1 \/ \d+$/);

  // Shift moves on after the decision.
  await page.keyboard.press("Shift+P");
  await expect(position).toHaveText(/^2 \/ \d+$/);
  await page.keyboard.press("ArrowLeft");
  await expect(pick).toHaveAttribute("aria-pressed", "true");

  // ↑ picks and ↓ rejects; Shift moves on.
  await page.keyboard.press("ArrowDown");
  await expect(reject).toHaveAttribute("aria-pressed", "true");
  await page.keyboard.press("ArrowUp");
  await expect(pick).toHaveAttribute("aria-pressed", "true");
  await page.keyboard.press("Shift+ArrowDown");
  await expect(position).toHaveText(/^2 \/ \d+$/);
  await page.keyboard.press("ArrowLeft");
  await expect(reject).toHaveAttribute("aria-pressed", "true");
  await page.keyboard.press("p");

  // Auto-advance: every decision moves on.
  await page.getByTestId("loupe-auto-advance").check();
  await page.keyboard.press("x");
  await expect(position).toHaveText(/^2 \/ \d+$/);
  await page.keyboard.press("ArrowLeft");
  await expect(reject).toHaveAttribute("aria-pressed", "true");
  await page.getByTestId("loupe-unflag").click();
  await expect(position).toHaveText(/^2 \/ \d+$/);
  await page.keyboard.press("ArrowLeft");
  await expect(pick).toHaveAttribute("aria-pressed", "false");
  await expect(reject).toHaveAttribute("aria-pressed", "false");

  // Leave the shared library as it was.
  await page.getByTestId("loupe-auto-advance").uncheck();
  await page.keyboard.press("0");
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("u");
});

test("shows the JPEG of a RAW+JPEG pair and switches to the RAW", async ({
  page,
  request,
}) => {
  // The oldest photo in the shared library, so End reaches it.
  const time = new Date("1985-05-05T10:00:00");
  await importFiles(request, {
    // The RAW's contents do not matter: only which file is asked for.
    "pair/IMG_6001.CR3": { bytes: Buffer.from("raw 6001"), time },
    "pair/IMG_6001.JPG": { bytes: jpeg(6001), time },
  });
  const tile = page
    .locator("[data-photo-id]")
    .filter({ has: page.locator('img[alt="IMG_6001.JPG"]') });
  const showPair = async () => {
    await page.locator("[data-photo-id]").first().click();
    await page.keyboard.press("End");
  };
  await page.goto("/");
  await showPair();
  // The grid shows the JPEG and marks that a RAW is there too.
  await expect(tile.getByTestId("tile-raw")).toBeVisible();
  await expect(tile.locator("img")).toHaveAttribute(
    "src",
    /\/grid\/\d+\/image$/,
  );

  await tile.dblclick();
  const fitted = page.getByTestId("loupe-image");
  const jpegButton = page.getByTestId("loupe-show-image");
  const rawButton = page.getByTestId("loupe-show-raw");
  await expect(fitted).toHaveAttribute("src", /\/preview\/\d+\/image$/);
  await expect(jpegButton).toHaveAttribute("aria-pressed", "true");
  await expect(jpegButton).toHaveText("JPEG");

  // The button and R switch between the two files.
  await rawButton.click();
  await expect(fitted).toHaveAttribute("src", /\/preview\/\d+\/raw$/);
  await expect(rawButton).toHaveAttribute("aria-pressed", "true");
  await page.keyboard.press("r");
  await expect(fitted).toHaveAttribute("src", /\/preview\/\d+\/image$/);

  // This RAW cannot be rendered, and the loupe says so.
  await rawButton.click();
  await expect(page.getByTestId("loupe-broken")).toContainText(
    "Can't show the RAW. Press R for the JPEG.",
  );
  await page.keyboard.press("r");
  await expect(page.getByTestId("loupe-broken")).toBeHidden();
  await page.keyboard.press("Escape");

  // The setting makes the RAW the default. The grid falls back to the
  // JPEG of a RAW it cannot show.
  await call(request, "update_settings", { pair_view: "raw" });
  try {
    await page.reload();
    await showPair();
    await expect(tile.locator("img")).toHaveAttribute(
      "src",
      /\/grid\/\d+\/image$/,
    );
    await tile.dblclick();
    await expect(fitted).toHaveAttribute("src", /\/preview\/\d+\/raw$/);
    await expect(rawButton).toHaveAttribute("aria-pressed", "true");
  } finally {
    await call(request, "update_settings", { pair_view: "image" });
  }
});
