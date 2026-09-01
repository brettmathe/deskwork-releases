# Deskwork

Desktop app for a markdown workspace: manage `inbox/` (create, edit, complete, delete) and browse `projects/` read-only. The workspace's markdown files are the only data store.

A *workspace* is any folder containing `inbox/` and `projects/`. Deskwork never bundles its contents -- it reads and writes them on disk at runtime. On first run it will find, clone, or create one for you.

Built with Tauri 2 (Rust backend) + Svelte 5 + TypeScript.

## Install

macOS only, universal (Apple Silicon and Intel). Grab the latest `.dmg` from
[Releases](https://github.com/brettmathe/deskwork-releases/releases/latest).

Deskwork is **not signed with an Apple Developer ID**, so Gatekeeper needs
handling on first launch. Which route you take decides how much:

### With the GitHub CLI (fewer steps)

```bash
cd ~/Downloads
gh release download --repo brettmathe/deskwork-releases --pattern "*.dmg"
hdiutil attach Deskwork_*_universal.dmg
cp -R /Volumes/Deskwork/Deskwork.app /Applications/
hdiutil detach /Volumes/Deskwork
open /Applications/Deskwork.app
```

No Gatekeeper prompt: the quarantine flag is attached by browsers and
LaunchServices, not by CLI downloads, so a file fetched this way never gets one.

### Manually, from the browser

1. Download the `.dmg` from the releases page.
2. Open it and drag **Deskwork** to Applications.
3. Clear the quarantine flag before first launch:

   ```bash
   xattr -dr com.apple.quarantine /Applications/Deskwork.app
   ```

4. Launch it.

Skip step 3 and macOS reports *"Deskwork is damaged and can't be opened"*. The
file is fine -- that is Gatekeeper's wording for an unsigned app it cannot
attribute. Right-click → Open no longer works around this on recent macOS; use
the command above, or System Settings → Privacy & Security → **Open Anyway**.

This friction disappears entirely if the app is ever signed with a Developer ID
and notarized. It applies to the first install only -- once running, updates are
signed with the app's own minisign key and install without prompting.

### First launch

The setup wizard appears, since a release build has no source tree to infer a
workspace from. See [First run](#first-run) for what it offers.

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

1. Bump the version: `npm run bump 0.2.0`.
2. Commit, then tag and push: `git tag v0.2.0 && git push origin v0.2.0`.

The version lives in six places across four files. `npm run bump` sets them all
and verifies afterwards that each one landed, failing loudly if any did not:

```
package.json                      0.2.0
package-lock.json (root)          0.2.0
package-lock.json (packages[""])  0.2.0
src-tauri/tauri.conf.json         0.2.0 (authoritative)
src-tauri/Cargo.toml              0.2.0
src-tauri/Cargo.lock              0.2.0
```

`tauri.conf.json` is the one that matters: the app reports it via `getVersion()`,
the updater compares against it, and the workflow reads it to name the release.
The rest are kept in step so nothing reports a stale number.

`npm run version:check` verifies the sites agree without changing anything, and
exits non-zero naming the offender if they don't. `npm run bump 0.2.0 --dry-run`
shows what would change.

The release workflow runs both guards before installing the Rust toolchain: the
consistency check, and an assertion that the pushed tag matches the version in
`tauri.conf.json`. So `git tag v0.3.0` on a commit still reading `0.2.0` fails
in seconds instead of publishing a release whose name, contents, and
`latest.json` disagree.

The workflow runs the test suite, verifies the bundle is clean, builds a
universal macOS binary, and publishes the `.dmg`, `.app.tar.gz`, signature, and
`latest.json` as a GitHub Release here. Installed copies pick it up on next launch.

`workflow_dispatch` runs a build without publishing.

### Known issue: DMG bundling hangs locally

Tauri's `bundle_dmg.sh` drives Finder over AppleScript to style the disk image,
and Tauri never passes the script's `--skip-jenkins` escape hatch. Without an
interactive GUI session holding Automation permission for Finder, that step
**hangs** rather than failing fast -- it has to be killed, and the run then
reports `error running bundle_dmg.sh`.

This affects local builds from a non-interactive shell. **CI is fine**: GitHub's
macOS runners have a usable session, and `v0.1.0` produced its `.dmg` there
without trouble.

The updater consumes `Deskwork.app.tar.gz` and never needs the DMG, so a local
build can simply skip it:

```bash
npm run tauri build -- --bundles app
```

The workflow caps the job at 60 minutes so a hang can't run away. If DMG
bundling ever does start failing on the runner, drop `,dmg` from the `args:`
line and first installs use the `.app.tar.gz` instead.

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
