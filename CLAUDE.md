# CLAUDE.md — working rules for this repo

**Picturesque** is a GPL-3.0-or-later desktop app that imports photos into an organized library and helps cull them (series, scores, best-in-series, local AI). Rust Cargo workspace + Tauri 2 + Svelte 5. Everything runs offline on the user's machine.

The architecture docs are the source of truth: `docs/architecture/` (`overview.rst`, `components.rst`, `data.rst`) and the ADRs in `docs/architecture/decisions/`. Read the relevant page before a non-trivial change. If code and docs disagree, stop and ask, or propose an ADR. Do not silently diverge. Docs are Sphinx/reStructuredText (ADR-0001); update them in the same change as the behavior.

Project skills (read when relevant, not up front): `development` (Rust practices), `architecture`, `documentation`, `gitflow`, `project-management`.

## Layout

| Path | Role |
|---|---|
| `crates/core` | Domain + pure logic. No I/O, no framework. |
| `crates/app` | Use cases, `api::Api::call` command dispatch, port traits in `src/ports/`. Depends on `core` only. |
| `crates/infra` | Port implementations: SQLite catalog (+ `catalog/migrations/`), decoding, `ort`, geocoder, XMP, critic. Native types stay here. |
| `crates/critic` | `picturesque-critic` helper process (llama.cpp), JSON lines via `core::critic_protocol` (ADR-0016). |
| `crates/devserver`, `src-tauri` | Thin entry points: HTTP (dev only) and Tauri IPC (ADR-0010). |
| `ui/` | Svelte 5 + TS + Vite. `src/lib/api` is the only backend access; `src/lib/api/generated` comes from ts-rs. Unit tests `*.test.ts` (vitest), e2e `ui/e2e/*.spec.ts` (Playwright). |
| `spikes/` | Throwaway experiments, excluded from the workspace. |

## Commands

```
make check           # fmt check, clippy -D warnings, cargo test, build, ui-check — run before declaring done
cargo test -p picturesque-core <filter>   # narrowest first; same for -app / -infra
make bindings-check  # after changing any type that crosses the API (ts-rs)
npm run check|lint|test -w ui            # UI type check, eslint+prettier, vitest
make e2e             # Playwright against devserver; stop manually started servers first
make dev-web         # devserver + Vite: the real UI in a headless browser
make docs-check      # strict Sphinx build + linkcheck, when docs changed
```

If `make test`/`make e2e` fail inside `ort-dylib` and `target/debug/libonnxruntime.so` exists, run `cargo test --workspace --all-targets && cargo test --workspace --doc` (or `cd ui && npx playwright test`) directly.

## Hard rules

1. **Layers (ADR-0003).** `core` has no I/O. `app` names no concrete database, decoder or runtime; it consumes port traits. Types from `rusqlite`, `ort`, libvips, LibRaw, llama.cpp and Tauri never appear in `app`/`core` signatures. `src-tauri` and `devserver` contain no business logic.
2. **Never lose a photo.** Sources are read-only; imports copy, hash and verify (ADR-0008). The only exception is the explicit, confirmed, re-verified source deletion of ADR-0014. Other destructive actions (moving rejects) are confirmed and undoable.
3. **Offline and private.** No network access except model downloads. No telemetry.
4. **One command API.** Every UI call goes through `Api::call`, reached only via `ui/src/lib/api`. Both transports stay thin adapters.
5. **Generated code is never hand-edited.** `ui/src/lib/api/generated` is written by `cargo test` (ts-rs); commit it and run `make bindings-check`.
6. **Catalog schema** changes only through a new numbered migration in `crates/infra/src/catalog/migrations/`; never edit a shipped one. Ask before changing the schema.
7. **Background work** (import, analysis, critique) is resumable from catalog state, cancellable, and keeps the UI responsive (ADR-0011, ADR-0012).
8. **Images: CC0, public domain or project-made only**, with source recorded (`docs/reference/third-party-data.rst`, fixture READMEs). Never commit anything derived from `spikes/` samples.
9. **No new dependency** without checking its license is GPL-3.0-or-later compatible and noting why in the commit message.
10. **UI performance.** The app renders in WebKitGTK: no `filter`/`backdrop-filter`/heavy shadows on repeated elements (tiles). Grids are virtualized.
11. **No display in the devcontainer.** Verify UI through devserver + Playwright. Run the real app only on Xvfb with `WAYLAND_DISPLAY` unset (`make smoke`).

## Conventions

- Rust: latest stable (see `rust-toolchain.toml`), edition 2024. Follow the crate's existing error types; no `unwrap` outside tests. Table-driven tests. Keep files under ~400 lines; split by responsibility.
- Svelte 5 runes; shared state in `*.svelte.ts` modules with a sibling `*.test.ts`. Icons from `heroicons`. No CSS framework.
- Git (see `gitflow` skill): branch `feature/<issue>_<title>` from `develop`; PRs target `develop`. Commit subjects start with `#<issue> `. Commit with `git commit -F <file>` so the `#` line survives.

## Model routing (token efficiency)

The main session runs Sonnet (`.claude/settings.json`) and is the **code writer**: it implements Rust and Svelte itself, with tests. Three subagents in `.claude/agents/` cover the rest. Spawning is cold-start and costs tokens, so use them only where a row below says so.

| Agent | Model | Use for |
|---|---|---|
| `architect` | Opus | Decisions that are expensive to reverse: layer boundaries/ports, catalog schema, command API/event shape, import safety, AI model/pipeline, ADRs. Also one review of a finished non-trivial change against the hard rules. Read-only; returns a short plan or review. |
| `tool-caller` | Haiku | GitHub/git (`gh`, branches, PRs, CI, merge, commits), diff digests, running `make check`/tests/`bindings-check`/`e2e`/`docs-check` and reporting failures, grep-style lookups, already-decided mechanical edits. |
| `docs-writer` | Sonnet | Sphinx docs, ADR files and reference pages from a decision already made. |

**Rules**

- Small or clear tasks: the main session just does them. Consult `architect` only for the "When unsure" list below or a non-trivial design; not for every issue.
- Brief completely: goal, files, acceptance criteria, exact command. Agents start cold.
- Delegate noisy output (test runs, `gh` and git queries, large diffs) to `tool-caller` so logs stay out of the main context. Ask for compact results: what changed, `file:line`, pass/fail.
- Haiku stops after two failed attempts or on any judgment call; Sonnet agents stop if the plan conflicts with the architecture docs or hard rules.
- Run independent agents in parallel (e.g. `docs-writer` while `tool-caller` runs the gates).
- Before declaring a code change done: `tool-caller` runs the gates; for non-trivial changes `architect` reviews the diff digest once.

**Context hygiene:** grep before read; read with offset/limit; don't re-read a file you just edited; narrowest test first, `make check` once at the end; `/clear` between issues.

## Workflow per issue

1. Restate the issue and its acceptance criteria; read the relevant architecture pages/ADRs; plan.
2. Tests first for `core` logic and use cases.
3. Smallest change that passes, written in the main session (`architect` first only if a decision is open).
4. `tool-caller`: narrow tests, then `make check` (+ `bindings-check`, `e2e`, `docs-check` as relevant).
5. `architect` review for non-trivial changes; fix findings.
6. Docs in the same change (via `docs-writer`).
7. Summarize: what changed, what was tested, what was deliberately left out.

## Autonomy (standing instruction from the maintainer)

For issues that already exist in GitHub (epic #217 and its milestone, or any issue the maintainer has pointed at), **do not ask whether to implement the next one.** Implement them one by one, in the order the epic lists them (dependencies first):

1. Branch from `develop`, implement in the main session, run the gates via `tool-caller`, open the PR into `develop`.
2. **Merge it yourself (squash) once CI is green and the PR is mergeable.** This is the explicit in-conversation merge instruction that the `gitflow` skill requires; it stays in force for every session.
3. Close the issue with a short summary, tick it in the epic, sync `develop`, and take the next issue without waiting.
4. Stop and tell the maintainer only when CI fails in a way you cannot fix, a decision under "When unsure" comes up, or the work is finished.

Limits that remain: never merge into `main` or cut a release; creating *new* issues, epics or milestones still needs the maintainer's approval (`project-management` skill); no force-push, no bypassing branch protection.

## When unsure

Ask before changing the catalog schema, the command API or event types, XMP sidecar output, anything that writes to or deletes user files, or which AI models ship. These are expensive to change later. Otherwise pick the simplest option consistent with the architecture docs and note the choice.
