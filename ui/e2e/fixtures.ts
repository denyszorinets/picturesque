import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

/** An 8×6 black JPEG (made with `vips black tiny.jpg 8 6`). */
const TINY_JPEG = Buffer.from(
  "/9j/4QC8RXhpZgAASUkqAAgAAAAGABIBAwABAAAAAQAAABoBBQABAAAAVgAAABsBBQABAAAAXgAAACgBAwABAAAAAgAAABMCAwABAAAAAQAAAGmHBAABAAAAZgAAAAAAAAA4YwAA6AMAADhjAADoAwAABgAAkAcABAAAADAyMTABkQcABAAAAAECAwAAoAcABAAAADAxMDABoAMAAQAAAP//AAACoAQAAQAAAAgAAAADoAQAAQAAAAYAAAAAAAAA/9sAQwAIBgYHBgUIBwcHCQkICgwUDQwLCwwZEhMPFB0aHx4dGhwcICQuJyAiLCMcHCg3KSwwMTQ0NB8nOT04MjwuMzQy/8AACwgABgAIAQERAP/EAB8AAAEFAQEBAQEBAAAAAAAAAAABAgMEBQYHCAkKC//EALUQAAIBAwMCBAMFBQQEAAABfQECAwAEEQUSITFBBhNRYQcicRQygZGhCCNCscEVUtHwJDNicoIJChYXGBkaJSYnKCkqNDU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6g4SFhoeIiYqSk5SVlpeYmZqio6Slpqeoqaqys7S1tre4ubrCw8TFxsfIycrS09TV1tfY2drh4uPk5ebn6Onq8fLz9PX29/j5+v/aAAgBAQAAPwD5/r//2Q==",
  "base64",
);

/** A JPEG whose bytes differ per `seed` (trailing bytes after the image data). */
export function jpeg(seed: number): Buffer {
  return Buffer.concat([
    TINY_JPEG,
    Buffer.from([seed & 0xff, (seed >> 8) & 0xff]),
  ]);
}

/** Camera details written into a test JPEG's EXIF. */
export interface Gear {
  make: string;
  model: string;
  lens?: string;
  /** Focal length in millimetres. */
  focal?: number;
}

/** A little-endian TIFF block with the IFD0 and Exif IFD tags of `gear`. */
function exifBlock(gear: Gear): Buffer {
  type Entry = { tag: number; type: number; count: number; data: Buffer };
  const ascii = (tag: number, text: string): Entry => {
    const data = Buffer.from(`${text}\0`, "latin1");
    return { tag, type: 2, count: data.length, data };
  };
  const ifd0: Entry[] = [ascii(0x010f, gear.make), ascii(0x0110, gear.model)];
  const exif: Entry[] = [];
  if (gear.focal !== undefined) {
    const data = Buffer.alloc(8);
    data.writeUInt32LE(Math.round(gear.focal * 10), 0);
    data.writeUInt32LE(10, 4);
    exif.push({ tag: 0x920a, type: 5, count: 1, data });
  }
  if (gear.lens) exif.push(ascii(0xa434, gear.lens));

  const ifdSize = (n: number) => 2 + n * 12 + 4;
  // Header, IFD0 (with the Exif pointer), Exif IFD, then long values.
  const ifd0At = 8;
  const exifAt = ifd0At + ifdSize(ifd0.length + 1);
  let dataAt = exifAt + ifdSize(exif.length);
  const out: Buffer[] = [Buffer.from("II*\0"), Buffer.alloc(4)];
  out[1]!.writeUInt32LE(ifd0At, 0);
  const extra: Buffer[] = [];
  const ifd = (entries: Entry[]) => {
    const head = Buffer.alloc(ifdSize(entries.length));
    head.writeUInt16LE(entries.length, 0);
    entries.forEach((e, i) => {
      const at = 2 + i * 12;
      head.writeUInt16LE(e.tag, at);
      head.writeUInt16LE(e.type, at + 2);
      head.writeUInt32LE(e.count, at + 4);
      if (e.data.length <= 4) e.data.copy(head, at + 8);
      else {
        head.writeUInt32LE(dataAt, at + 8);
        extra.push(e.data);
        dataAt += e.data.length;
      }
    });
    return head;
  };
  const pointer = Buffer.alloc(4);
  pointer.writeUInt32LE(exifAt, 0);
  out.push(
    ifd([...ifd0, { tag: 0x8769, type: 4, count: 1, data: pointer }]),
    ifd(exif),
    ...extra,
  );
  return Buffer.concat(out);
}

/**
 * Like `jpeg(seed)`, with `gear` in its EXIF (make, model and, when given,
 * lens and focal length).
 */
export function jpegWithGear(seed: number, gear: Gear): Buffer {
  const tiff = exifBlock(gear);
  const app1 = Buffer.alloc(4);
  app1.writeUInt16BE(0xffe1, 0);
  app1.writeUInt16BE(2 + 6 + tiff.length, 2);
  // Where the tiny JPEG's own EXIF segment (right after SOI) ends.
  const afterExif = 4 + TINY_JPEG.readUInt16BE(4);
  return Buffer.concat([
    TINY_JPEG.subarray(0, 2),
    app1,
    Buffer.from("Exif\0\0", "latin1"),
    tiff,
    TINY_JPEG.subarray(afterExif),
    Buffer.from([seed & 0xff, (seed >> 8) & 0xff]),
  ]);
}

/** Creates a temporary folder containing `files` (name → contents). */
export function folder(files: Record<string, Buffer>): string {
  const dir = mkdtempSync(join(tmpdir(), "picturesque-e2e-"));
  for (const [name, contents] of Object.entries(files)) {
    const path = join(dir, name);
    mkdirSync(join(path, ".."), { recursive: true });
    writeFileSync(path, contents);
  }
  return dir;
}
