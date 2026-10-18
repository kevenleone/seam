# Agent Instructions

This file is the single source of truth for agent instructions in this repository.

## What This Is

Seam is a desktop git client, Tauri 2 over the user's own `git`, for everyday
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
| `pnpm app:dev`               | Run the app with hot reload as **Seam Dev** (Vite on port 1420 plus the Tauri window).        |
| `pnpm test`                  | `cargo test`: parser unit tests and integration tests against throwaway git repos.            |
| `pnpm lint`                  | ESLint. It passes clean; keep it that way.                                                    |
| `pnpm format`                | Prettier over everything. Required before committing.                                         |
| `pnpm typecheck`             | Typecheck the renderer.                                                                       |
| `cargo fmt` / `cargo clippy` | In `src-tauri/`. CI runs `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`. |
| `pnpm app:build`             | Release `.app` and `.dmg` in `src-tauri/target/release/bundle/`.                              |

`pnpm app:dev` merges `src-tauri/tauri.dev.conf.json` over the release config:
its own name, identifier (`app.seam.desktop.dev`, so its own prefs and recents)
and icons (`src-tauri/icons/dev/`), so it runs beside an installed Seam. Host
strings take the name from `package_info()`, never a literal "Seam".

Seam Dev rebuilds from the checkout it runs in, so switching that checkout's
branch from inside it restarts the app on the other branch's code (and fails
on branches older than the dev config). Point Seam Dev at another worktree or
repository when trying branch switches.

`cargo test` and `cargo clippy` need `dist/` to exist (`tauri::generate_context!`
embeds it), so run `pnpm build` once in a fresh checkout.

Release builds do not strip (`[profile.release]` in `src-tauri/Cargo.toml`):
the macOS 27 toolchain's `strip` writes Mach-O files dyld refuses to load. Leave
it off until that is fixed upstream.

## Architecture

Two processes, two kinds of window.

**Host and engine** (`src-tauri/`): Rust. `src/git/` is the whole git engine and
has no Tauri dependency, so it is tested directly:

- `process.rs`: runs git with the login shell's environment (captured once, so
  hooks find node and friends when the app is launched from Finder).
- `parse.rs`: porcelain v2 status, unified diffs, log, refs, stashes, numstat.
- `patch.rs`: patches holding only some of a diff's lines, for staging,
  unstaging and discarding single lines.
- `rewrite.rs`: rename, remove and squash any commit with a scripted `rebase -i`.
- `repo.rs`: every operation, each returning an `Undoable`.
- `ops.rs`: the `Op` enum the renderer sends and the per-repository undo/redo stacks.

`src/ai.rs` is every model Seam can talk to, natively, with no sidecar and no SDK:
the coding agents' own CLIs, headless and read-only in the repository
(`claude -p`, `codex exec --sandbox read-only`, `opencode run`), and the HTTP
APIs of Anthropic, OpenAI, Google Gemini and any OpenAI-compatible endpoint
(Ollama, LM Studio, OpenRouter), called with `reqwest`. API keys live in the
macOS Keychain (`keyring`), never in a file or in the web views. A new provider
is a `Provider` variant plus its arm in `ask_harness` or `ask_api`.

`src/review.rs` is the AI review on top of it: it picks the scope (the branch
against `main`/`master`/…, else uncommitted changes), builds the prompt from the
review style in Settings › AI, asks for one JSON object of findings and parses
it leniently. The last review is saved in `.git/seam/review.json`.
`SEAM_LIVE_AI=claude cargo test --test review -- --ignored` runs a real model
end to end; any provider id works there.

`src/visual.rs` is the Visual tab on the same footing: a recap of the review's
scope, or a plan for a change the user describes, asked for as one JSON document
of typed blocks (wireframe screens, diagrams, decisions, contracts, key diffs,
test steps). Parsing grounds it in the diff: file stats come from git, and diff
blocks naming a file outside it are dropped. Recaps and plans are kept in
`.git/seam/recaps/` and `.git/seam/plans/` (the newest 30 of each, listed by an
`index.json`); a recap remembers its branch and is shown only on it. The renderer draws it in
`src/components/visual/` and exports it to HTML, PDF, Markdown and Excalidraw
(`src/lib/visualExport.ts`). `SEAM_LIVE_AI=claude cargo test --test visual -- --ignored`
runs both kinds live; `SEAM_LIVE_OUT=<dir>` keeps the JSON they produced.

`src/overview.rs` is the Overview tab. Its facts (stack, languages, activity,
hot paths, scripts) come from git and the manifests with no AI, so the tab is
never empty. The AI overview is three fixed sections (architecture, modules,
where to start) of Visual's blocks, grounded in `git ls-files`. Each section
lists the paths it covers and the overview keeps the commit it was written at,
so drift is a `git diff --name-only` away and an update rewrites only the
sections it touches. Overviews are kept in `.git/seam/overviews/`.
`SEAM_LIVE_AI=claude cargo test --test overview -- --ignored` writes one live.

`src/tray.rs` is the menu bar icon (recent projects, review the last-focused
repository) and `src/prefs.rs` what the host remembers in the app data folder
(the AI configuration and recent repositories). The host owns both because the
tray needs them with no window open. Closing the last window keeps Seam running;
only Quit (⌘Q or the tray) exits.

`src/lib.rs` is the Tauri side: commands, the welcome window and one window per
repository (labelled by a hash of its path), the menu, the file watcher, and
folders opened from Finder or the Dock. Actions on one repository are serialised
by a per-repository lock, undo and redo included.

**Renderer** (`src/`): React 19, Zustand, Tailwind v4, Vite. One `index.html`;
each window loads it with `?view=welcome` or `?repo=<path>` (`src/main.tsx`).

- **`src/lib/api.ts`** and **`src/lib/types.ts`** mirror the Rust commands and
  models one to one. Change both sides together.
- **`src/lib/store.ts`**: a window's state. `perform(op)` is the only way an
  action reaches git; it queues behind the previous action, then refreshes.
- **`src/components/DiffView.tsx`**: the virtualised diff shared by Changes
  and History. Large diffs must stay virtualised.

Windows are created hidden and show themselves after the first paint
(`main.tsx`); the host shows them anyway after 1.5 s. Keep both halves, or a new
window flashes white.

## Styling

Tailwind CSS v4 over Seam's tokens (`--app`, `--panel`, `--add-bg`, …), bridged
in the `@theme inline` block of `src/styles.css`.

- A theme is a whole token map in `src/lib/themes.ts`. Seam Dark and Seam Light
  are hand-tuned to the design canvas; the rest are seeds (the dozen colours a
  palette publishes) expanded by `buildTokens`. Every theme keeps a **blue for
  additions and an orange for deletions**, never green and red.
- `applyTheme` (`src/lib/settings.ts`) paints the chosen theme and accent onto
  the root and caches the map; `public/theme.js` repaints that cache before the
  bundle loads, so the first frame is already right. A new token is added to
  `SEAM_DARK`, `SEAM_LIGHT` and `buildTokens`, and bridged in `@theme inline`.
  Syntax colours are the seven `--syn-*` tokens, mapped onto highlight.js classes in
  `styles.css`; `src/lib/highlight.ts` bundles only the languages it registers.
- Colours come from tokens (`bg-panel`, `text-muted`), never a hex in a
  component, and there is no `dark:` variant.
- An inline `style` is right only for values a class cannot carry (a computed
  length, a virtualised row offset, a swatch previewing a colour).

## Settings

`src/components/settings/` is the settings sheet (⌘,), after Lore's: a rail of
panes with search, title/description rows with a trailing control. Interface
preferences live in `localStorage` (`src/lib/settings.ts`, shared by every
window); what the host needs with no window open (the AI configuration and
recent repositories) lives in `src-tauri/src/prefs.rs`. A repository window
shows settings as a sheet; elsewhere they open in their own window.

## Code Style

- `interface` for object shapes, `type` for unions/aliases
- Early returns over nested conditionals; optional chaining (`?.`) and nullish coalescing (`??`)
- Explicit return types on exported functions
- Functional components only; no class components
- No `any`: TypeScript strict mode enforced everywhere
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
when the code alone would let someone break something important; otherwise
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
- Do **not** add AI attribution footers anywhere: no "🤖 Generated with Claude Code" (or similar) in commit messages, PR descriptions, or issue comments.
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
- Do not add hard rules by commit type; use your judgment
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
