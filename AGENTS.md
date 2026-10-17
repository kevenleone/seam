# Agent Instructions

This file is the single source of truth for agent instructions in this repository.

## What This Is

Seam is a desktop git client — Tauri 2 over the user's own `git` — for everyday
work on a repository: stage single lines, commit, stash, rename or remove
commits, and undo any of it with ⌘Z. It is the first pass of a larger design
(AI review, agent sessions, pull requests, a three-way conflict view and a
`seam` CLI come later); build toward that, not away from it.

The principles that decide arguments, in the order they usually come up:

- **Git is the engine.** Every read and write shells out to the git the user
  already has (`src-tauri/src/git/process.rs`), so their config, hooks,
  credential helpers and SSH behave exactly as in Terminal. Do not add libgit2
  or reimplement a git operation in Rust or TypeScript.
- **Every action is undoable, or says it isn't.** A mutating method in
  `repo.rs` returns an `Undoable` that records what it needs to reverse itself
  (an index tree, file blobs, or the old tip kept alive under
  `refs/seam/undo/*`). A new action ships with its undo and a test that undoes
  it. Push is the one exception, and the toast says so.
- **Never lose what the user had.** Rewrites run with `--autostash`; discards
  snapshot file contents first; a failed operation leaves git's own
  in-progress state for the banner to abort or continue.
- **No editor ever opens.** History rewrites write their own `rebase -i` todo
  list (`rewrite.rs`) and set `GIT_EDITOR` to `true`.
- **The renderer only describes intent.** It sends an `Op` (`ops.rs`) and gets
  an outcome back; it never builds a git command line.

The README is user-facing; read it for what the app does. This file is for how
to work on it.

## Commands

`pnpm` only.

| Command                      | What it does                                                                                  |
| ---------------------------- | --------------------------------------------------------------------------------------------- |
| `pnpm app:dev`               | Run the app with hot reload (Vite on port 1420 plus the Tauri window).                        |
| `pnpm test`                  | `cargo test`: parser unit tests and integration tests against throwaway git repos.            |
| `pnpm lint`                  | ESLint. It passes clean; keep it that way.                                                    |
| `pnpm format`                | Prettier over everything. Required before committing.                                         |
| `pnpm typecheck`             | Typecheck the renderer.                                                                       |
| `cargo fmt` / `cargo clippy` | In `src-tauri/`. CI runs `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`. |
| `pnpm app:build`             | Release `.app` and `.dmg` in `src-tauri/target/release/bundle/`.                              |

`cargo test` and `cargo clippy` need `dist/` to exist (`tauri::generate_context!`
embeds it), so run `pnpm build` once in a fresh checkout.

Release builds do not strip (`[profile.release]` in `src-tauri/Cargo.toml`):
the macOS 27 toolchain's `strip` writes Mach-O files dyld refuses to load. Leave
it off until that is fixed upstream.

## Architecture

Two processes, two kinds of window.

**Host and engine** (`src-tauri/`) — Rust. `src/git/` is the whole git engine and
has no Tauri dependency, so it is tested directly:

- `process.rs` — runs git with the login shell's environment (captured once, so
  hooks find node and friends when the app is launched from Finder).
- `parse.rs` — porcelain v2 status, unified diffs, log, refs, stashes, numstat.
- `patch.rs` — patches holding only some of a diff's lines, for staging,
  unstaging and discarding single lines.
- `rewrite.rs` — rename, remove and squash any commit with a scripted `rebase -i`.
- `repo.rs` — every operation, each returning an `Undoable`.
- `ops.rs` — the `Op` enum the renderer sends and the per-repository undo/redo stacks.

`src/lib.rs` is the Tauri side: commands, the welcome window and one window per
repository (labelled by a hash of its path), the menu, the file watcher, and
folders opened from Finder or the Dock. Actions on one repository are serialised
by a per-repository lock, undo and redo included.

**Renderer** (`src/`) — React 19, Zustand, Tailwind v4, Vite. One `index.html`;
each window loads it with `?view=welcome` or `?repo=<path>` (`src/main.tsx`).

- **`src/lib/api.ts`** and **`src/lib/types.ts`** mirror the Rust commands and
  models one to one. Change both sides together.
- **`src/lib/store.ts`** — a window's state. `perform(op)` is the only way an
  action reaches git; it queues behind the previous action, then refreshes.
- **`src/components/DiffView.tsx`** — the virtualised diff shared by Changes
  and History. Large diffs must stay virtualised.

Windows are created hidden and show themselves after the first paint
(`main.tsx`); the host shows them anyway after 1.5 s. Keep both halves, or a new
window flashes white.

## Styling

Tailwind CSS v4 with Seam's tokens in `src/styles.css`, taken from the design
canvas. Dark is the reference theme; light mirrors it.

- Colours come from tokens (`bg-panel`, `text-muted`, `bg-add-bg`) — never a
  hex in a component. A new colour is added to **both** `[data-theme]` blocks
  and bridged in `@theme inline`.
- There is no `dark:` variant. A theme swaps the whole token map.
- Additions are blue and deletions orange, never green and red, so the two stay
  distinct for colour-blind readers.
- `public/theme.js` applies the saved theme before anything else loads, so the
  first paint is already right.
- An inline `style` is right only for values a class cannot carry (a computed
  length, a virtualised row offset).

## Code Style

- `interface` for object shapes, `type` for unions/aliases
- Early returns over nested conditionals; optional chaining (`?.`) and nullish coalescing (`??`)
- Explicit return types on exported functions
- Functional components only; no class components
- No `any` — TypeScript strict mode enforced everywhere
- No single-letter parameters (ESLint enforces it) and no abbreviations in new identifiers
- Rust: `cargo fmt` defaults; clippy clean with `-D warnings`

### Imports

- ES modules only (`import`/`export`)
- Type-only imports use the `type` keyword: `import type { Commit } from '...'`
- Import order and object keys are sorted by `eslint-plugin-perfectionist`; `pnpm lint:fix` applies it

### Naming

- Components and their files: PascalCase (`DiffView.tsx`, `ChangesView.tsx`)
- Other modules: camelCase (`store.ts`, `settings.ts`)
- Hooks: camelCase starting with `use` (`useAppVersion`)
- True constants: UPPER_SNAKE_CASE

### Testing

- A new git operation gets an integration test in `src-tauri/tests/repo.rs`
  that runs it against a throwaway repository and undoes it.
- A parser change gets a fixture in the `#[cfg(test)]` module of `parse.rs`.

### Comments

**The default is no comment.** Most code needs none, and most comments that get
written are restating what the code already says. A comment earns its place only
when the code alone would let someone break something important — otherwise
leave it out and let the code speak.

Write one when, and only when:

- Removing it would let a reader silently break correctness or security (e.g. why
  webhook signatures verify against the raw body, not the re-serialized JSON).
- Ordering or placement is load-bearing and not obvious from reading top to bottom.
- The code looks wrong or redundant but is deliberate (a loop that intentionally
  does not short-circuit, an early return that is safe despite appearances).
- An external contract is being matched (a provider's wire format, a spec).

Do **not** write one to:

- Restate what the next line does (`// Only whether a secret exists` above
  `hasSecret: !!secretCiphertext`).
- Label a section, a constant, or an obviously-named function.
- Explain where code was placed or which folder it belongs to.
- Narrate a change, decision history, or what the code used to do.
- Summarise a file at the top of it. A module's job should be evident from its
  name and its exports.
- Document a parameter, field or return whose name already says it.
- Record why a bug happened. That belongs in the commit message; the fix belongs
  in a test that fails without it.

**Prefer one tight line over three, and no comment over one tight line.** If a
comment needs a paragraph, the code probably needs a better name or a smaller
function instead. A file where comments outnumber a handful of lines is a file
that has been narrated rather than written.

## Commit Rules

Use [Conventional Commits](https://www.conventionalcommits.org/): `<prefix>: <description>`.

- Run `pnpm format` before committing; if Prettier modifies files, stage them and add a final `chore: Source Format` commit as the last commit in the sequence.
- Do **not** add `Co-authored-by:` trailers.
- Do **not** add AI attribution footers anywhere — no "🤖 Generated with Claude Code" (or similar) in commit messages, PR descriptions, or issue comments.
- Avoid commit bodies/footers unless the change has a breaking or high-impact side effect.
- Split commits by logical area; keep each commit focused on one concern.

| Prefix     | Use when                           |
| ---------- | ---------------------------------- |
| `feat`     | New feature                        |
| `fix`      | Bug fix                            |
| `refactor` | No bug fix, no new feature         |
| `style`    | Formatting/whitespace only         |
| `docs`     | Documentation only                 |
| `test`     | Adding/updating tests              |
| `chore`    | Maintenance, deps, tooling         |
| `perf`     | Performance improvement            |
| `ci`       | CI/CD changes                      |
| `build`    | Build system or dependency changes |

### Commit Splitting

- Split changes into one or more commits by area, context, or logical grouping
- Do not add hard rules by commit type — use your judgment
- Keep each commit focused on a single concern

### Co-Author Trailer

- Never add `Co-authored-by:` for the agent under any circumstances

### Formatting Workflow

1. Before committing, run `pnpm format` at the project root
2. If `prettier` modified any files, stage those changes and add a final commit:
    ```
    chore: Source Format
    ```
3. This formatting commit must be the last in the sequence
