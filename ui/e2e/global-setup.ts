import { mkdirSync, rmSync } from "node:fs";
import { LIBRARY } from "./helpers";

/** Every run starts with an empty library (the dev server's catalog is reset too). */
export default function globalSetup() {
  rmSync(LIBRARY, { recursive: true, force: true });
  mkdirSync(LIBRARY, { recursive: true });
}
