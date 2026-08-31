# Deskwork

Desktop app for a markdown workspace: manage `inbox/` (create, edit, complete, delete) and browse `projects/` read-only. The workspace's markdown files are the only data store.

A *workspace* is any folder containing `inbox/` and `projects/`. Deskwork never bundles its contents -- it reads and writes them on disk at runtime. On first run it will find, clone, or create one for you.

Built with Tauri 2 (Rust backend) + Svelte 5 + TypeScript.

## Run

```bash
npm install
npm run tauri dev
```

Requires Node and the Rust toolchain (`rustup`). Point the app at a workspace with the `DESKWORK_REPO` env var, the setup wizard, or the folder picker in Settings.

To work on the wizard itself, force it with `DESKWORK_FORCE_SETUP=1 npm run tauri dev`.

## First run

With no saved workspace, the app opens a setup wizard instead of the main shell. A *workspace* is any folder with `inbox/` and `projects/` in it. The wizard offers four ways to get one:

| Option | What it does |
|--------|--------------|
| **Found on this Mac** | Scans one level under `~/Projects`, `~/Developer`, `~/src`, `~/code`, `~/Documents`, `~/repos`, and `~` for valid workspaces and lists them with their `origin` remote |
| **Choose a folder…** | Native folder picker, validated before it's accepted |
| **Clone from a remote** | Clones into a chosen parent folder — tries `gh repo clone` first so private GitHub repos work off the CLI's stored token, then falls back to `git clone` |
| **Create a new workspace** | Scaffolds `inbox/`, `completed/`, `projects/` (with the registry table and `_template.md`), a README and `.gitignore`, then optionally `git init`s on `main` and makes the first commit |

Every subprocess runs with `GIT_TERMINAL_PROMPT=0` and SSH `BatchMode=yes`, so a missing credential fails with a readable message instead of hanging the window on a prompt no one can see.

Creating a workspace leaves it with no remote. The Repository panel will read its status fine; `git remote add origin …` when you're ready to push. If git has no `user.name`/`user.email`, the folder is still created and initialized — the wizard says the commit was skipped rather than failing.

## Build a release app

```bash
npm run tauri build
```

Produces `src-tauri/target/release/bundle/macos/Deskwork.app`.

## Updates

The app checks for a new version on launch (silently — an offline machine or a
dev build never raises an error nobody asked for) and offers to install it from
the sidebar or Settings. Updates are signed with minisign; a bundle that fails
signature verification is refused by the updater.

### What is published

Releases are built and published from this repository by
`.github/workflows/release.yml`. Because the workflow and the release live in
the same repo, the automatic `GITHUB_TOKEN` is sufficient -- there is no
personal access token to create or rotate.

Two disclosure rules the tooling enforces, since this repo is public:

- `tests/no_hardcoded_workspace.rs` fails the build if a repository URL literal
  reaches the shipped frontend. A prefilled clone URL would disclose a private
  workspace's path to everyone who downloads the app, which is why that field
  ships empty with a placeholder.
- The release body is written literally in the workflow, and
  `generateReleaseNotes` is pinned false so commit subjects never become the
  public release text.

### Cutting a release

1. Bump `version` in `src-tauri/tauri.conf.json` (and `package.json` to match).
2. Tag and push: `git tag v0.2.0 && git push origin v0.2.0`.

The workflow runs the test suite, verifies the bundle is clean, builds a
universal macOS binary, and publishes the `.dmg`, `.app.tar.gz`, signature, and
`latest.json` as a GitHub Release here. Installed copies pick it up on next launch.

`workflow_dispatch` runs a build without publishing.

### Known issue: DMG bundling

Tauri's `bundle_dmg.sh` drives Finder over AppleScript to style the disk image,
and Tauri never passes the script's `--skip-jenkins` escape hatch. Without an
interactive GUI session holding Automation permission for Finder, that step
**hangs** rather than failing fast (locally it has to be killed; the run then
reports `error running bundle_dmg.sh`).

The updater does not need the DMG — it consumes `Deskwork.app.tar.gz` — so a
local `--bundles app` build always works:

```bash
npm run tauri build -- --bundles app
```

The workflow caps the job at 60 minutes so a hang can't run away. If DMG
bundling turns out to fail on the runner, drop `,dmg` from the `args:` line and
first installs use the `.app.tar.gz` instead.

### Required secrets

| Secret | What it is |
|--------|------------|
| `TAURI_SIGNING_PRIVATE_KEY` | Contents of the minisign private key (`~/.tauri/deskwork.key`) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Its password (empty string if generated without one) |

The matching public key is committed in `tauri.conf.json` — that one is meant to
be public. **The private key is not in this repo and must never be**: losing it
means no existing install can verify a future update, and leaking it lets anyone
publish an update your users will trust and install.

## Tests

```bash
cd src-tauri && cargo test
```

Covers the inbox parser (Slack-scanner format, undated files, malformed bodies), the slug/filename convention (mirrors `agents/slack-task-scanner/scan.js`), the registry-table and unfenced-metadata parsers, atomic writes and collision suffixes, a full create→edit→complete→delete round-trip in a sandbox, setup-wizard scaffolding (URL parsing, destination guards, idempotent scaffold, `git init`, workspace detection), a created workspace driven through the whole task lifecycle, the public-release identifier guard, and and parser checks against a checked-in fixture workspace.

## How actions map to files

| Action | Filesystem effect |
|--------|-------------------|
| Complete | `inbox/X.md` renamed to `completed/X.md` (git detects a rename); `-2` suffix if the name is taken |
| Create | `inbox/YYYY-MM-DD-{slug}.md` with `# Title` / `Added:` header (same convention as the Slack scanner) |
| Edit | Atomic in-place write (temp file + rename) |
| Delete | File removed after a native confirm — recoverable via git until committed |
| Projects | Read-only; `.html` artifacts open in the default browser, other files open externally |

All writes are confined to `inbox/` and `completed/` by construction; `docs/`, `pages-static/`, and `.git/` are unreachable.

## Git sync (sidebar Repository panel)

The sidebar shows the current branch, ahead/behind counters (fetched every 5 minutes and on window focus), and the number of uncommitted task changes.

- **Pull** — `git pull --rebase --autostash`; shown whenever origin has new commits (e.g. the pages-static bot commit after every push). A failed rebase is aborted automatically so the repo is never left mid-operation.
- **Commit & Push** — stages **only** `inbox/` and `completed/`, commits with an editable prefilled message, pulls first if origin moved, then pushes. Changes elsewhere in the repo are shown as "+N outside tasks" and are never committed by the app.

Git runs as your user (`git` on PATH), so your normal SSH/keychain credentials apply.

## Layout

- `src-tauri/src/repo.rs` — workspace resolution, path confinement, atomic writes
- `src-tauri/src/setup.rs` — first-run detection, clone, and workspace scaffolding
- `src-tauri/src/inbox.rs` — inbox parsing + all mutating commands
- `src-tauri/src/projects.rs` — registry table, README metadata, artifact tree (read-only)
- `src/routes/+page.svelte` — three-pane shell (sidebar / list / detail)
- `src/lib/SetupWizard.svelte` — first-run wizard (detect / pick / clone / create)
- `src/lib/updater.svelte.ts` — update check/download state; `UpdateBanner.svelte` is its sidebar surface
- `src/lib/` — panes, modal, markdown rendering (marked + DOMPurify), theme
