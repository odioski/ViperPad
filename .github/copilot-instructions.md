# Copilot instructions for ViperPad

## Build and test commands

| Task | Command | Notes |
| --- | --- | --- |
| Run the app locally | `cargo run --release` | Serves the UI at `http://127.0.0.1:4173` and does not open a browser. |
| Build the release binary | `cargo build --release --locked` | Matches the release build used by the AppImage script. |
| Run all Rust tests | `cargo test` | Current Rust tests live in `src/main.rs`. |
| Run one Rust test | `cargo test renders_common_markdown` | Use the test name to target a single unit test. |
| Install editor bundling deps | `npm ci` | Use before Node-based editor work or the bundle consistency check. |
| Rebuild the embedded editor bundle | `npm run build` | Bundles `web/editor.js` into the checked-in `src/editor.bundle.js`. |
| Verify the checked-in editor bundle | `npm test` | Rebuilds `src/editor.bundle.js` and fails if `git diff --exit-code -- src/editor.bundle.js` is not clean. |
| Build the Flatpak bundle | `./build-flatpak.sh` | Generates an untracked `packaging/flatpak/cargo-sources.json` first and writes `dist/ViperPad.flatpak`. |
| Prepare and validate Flathub files | `./build-flathub.sh v0.1.0` | Requires a clean worktree and a pushed tag. |
| Build the AppImage | `./build-appimage.sh` | Produces a release binary, validates desktop metadata, and writes the AppImage under `dist/`. |

## High-level architecture

- `src/main.rs` is the whole backend: an Axum server that embeds `src/app.html`, `src/editor.bundle.js`, and `assets/viperpad-splash.png` with `include_*` macros and serves them directly.
- File state is intentionally ephemeral. `AppState` stores `FileEntry` values in an in-memory `HashMap<u64, FileEntry>`, so `/view/{id}` URLs and uploaded contents disappear when the process stops.
- The browser UI lives mostly in `src/app.html`. Its inline script owns file picking, remote URL loading, viewer URL copy, edit/preview mode switching, save behavior, and polling local files for changes through the File System Access API.
- `web/editor.js` is the editable source for the CodeMirror integration. It exposes `window.LiveEditor`, dynamically loads a language mode from `@codemirror/language-data`, and is bundled into `src/editor.bundle.js`, which Rust serves as a static asset.
- Preview and edit flows are split:
  - Edit mode fetches `/api/files/{id}/raw` and mounts CodeMirror through `LiveEditor`.
  - Preview mode uses `/api/files/{id}`. Markdown is rendered server-side with `pulldown-cmark`, text/code are HTML-escaped into `<pre>`, and HTML is returned directly.
  - Code files preview inside the CodeMirror view; other rendered content is shown in a sandboxed `<iframe>`.
- Remote URL support goes through `PUT /api/remote`. The server downloads UTF-8 text only, caps size at 10 MB, infers the file kind from extension/content type, and stores the final response URL as `base_url` so relative links/assets can resolve through an injected `<base>` tag.
- Packaging is script-driven. `build-flatpak.sh`, `build-flathub.sh`, and `build-appimage.sh` read metadata from `Cargo.toml` and `packaging/`, while `scripts/generate-flatpak-sources.sh` pins and caches the Flatpak cargo generator under `.cache/`.

## Product direction

- Treat `MOTIVATION.md` as part of the initialization context for product intent and roadmap-level goals.
- The project aims to become a fast, focused developer pad rather than a heavyweight all-in-one IDE.
- Prioritize excellent editing, instant preview, local-first workflows, sharp code tools without bloat, and a "scratchpad for real projects" experience.

## Key repository conventions

- Treat `src/editor.bundle.js` as a committed build artifact, not a generated-by-CI-only file. Any change to `web/editor.js` or the CodeMirror dependency set should be followed by `npm run build` and usually `npm test`.
- Keep the split between the frontend shell and editor integration:
  - `src/app.html` holds the overall UI and API orchestration.
  - `web/editor.js` should stay focused on CodeMirror setup and editor lifecycle.
- Preserve the app's text-only contract. Both local uploads and remote fetches expect UTF-8 text, and both local and remote file payloads are capped at 10 MB.
- Preserve the current preview safety model when touching rendering paths: HTML is sandboxed in an iframe, plain text/code paths use `escape_html`, and remote HTML relies on injected `<base>` handling for relative assets.
- Viewer links are not durable identifiers. Do not design features around `/view/{id}` surviving a restart unless you also add persistence to `AppState`.
- Packaging scripts assume repository-managed metadata locations: manifests live under `packaging/`, generated deliverables go to `dist/`, intermediate packaging output goes under `build/`, and `build-flathub.sh` aborts unless the git worktree is clean.
