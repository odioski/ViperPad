# ViperPad

A small Rust app for viewing and editing Markdown, HTML, text, and source-code files in one browser window. It provides a local viewer URL and automatically refreshes when the selected file changes. A bundled [CodeMirror](https://codemirror.net/) build handles both source preview and editing with unrestricted language detection, line numbers, search, folding, bracket matching, indentation, and undo history.

ViperPad is mostly an editor: think Scratchpad or a lightweight online editor, but local, offline-capable, and without a separate runtime window. The Rust process supplies the local server while the browser is the interface.

No internet connection is required for local files. HTTP and HTTPS URLs can also be opened through the address field.

The centered empty-state artwork is bundled from `assets/viperpad-splash.png`; it is available offline with the rest of the interface.

## Run

```sh
cargo run --release
```

The server listens at `http://127.0.0.1:4173` without launching a browser. Open that URL manually. Press `Ctrl+C` in the terminal to stop it.

Chrome and Edge support live file watching and the system save-file picker best. Firefox does not support the same local file save flow, and Safari may also be limited. Press **Edit**, make changes, and use **Save** or `Ctrl+S` to choose a destination.

Remote URLs are limited to UTF-8 text files of 10 MB or less. They can be viewed and opened in the editor, but are read-only because ViperPad has no credentials or remote write protocol.

Viewer URLs and uploaded file contents are held in memory and disappear when the app stops. HTML is rendered in a sandboxed iframe.

## Rebuild the bundled editor

The generated `src/editor.bundle.js` is checked in as an expanded, non-minified bundle and embedded by Rust, so normal builds only require Cargo. Node.js is needed only when changing the editor integration:

```sh
npm install
npm run build
npm test
```

`npm test` rebuilds `src/editor.bundle.js` and fails if the checked-in bundle does not match the generated output. The Node.js GitHub Actions workflow uses that check across supported Node versions.

## Linux packaging

Install every host tool and Flatpak SDK required by both packaging workflows:

```sh
./install-deps.sh
```

The installer supports Fedora, Debian/Ubuntu, Arch Linux, and openSUSE. It installs:

- Rust, Cargo, a C/C++ toolchain, binutils, Git, curl, and `file`
- Node.js and npm for rebuilding the embedded CodeMirror bundle
- Flatpak, `flatpak-builder`, Freedesktop Platform/SDK 25.08, the matching Rust SDK extension, and `org.flatpak.Builder`
- Python 3, pip, and venv support for the pinned Flatpak Cargo source generator; Python dependencies are installed in the repository's `.venv`
- desktop-file and AppStream validators, `patchelf`, and SquashFS tools

On a machine where the system and Flatpak dependencies are already installed, run `./install-deps.sh --venv-only` to create or update only the repository's `.venv`.

The Rust executable dynamically uses only the Linux base ABI: glibc, `libm`, `libgcc_s`, and the ELF loader. These libraries must not be bundled in an AppImage because they are supplied by the target distribution. For maximum AppImage compatibility, build on the oldest supported Linux distribution.

### Flatpak

```sh
./build-flatpak.sh
flatpak install --user dist/ViperPad.flatpak
flatpak run io.github.odioski.ViperPad
```

The build requires a clean worktree, pins the local Git `HEAD`, generates its Cargo source list, compiles with Freedesktop SDK 25.08, and writes `dist/ViperPad.flatpak`.

### Flathub validation

For the unreleased `main` branch, run:

```sh
./build-flathub.sh
```

You can pass a remote branch or tag as the first argument. The script requires a clean worktree, resolves the requested ref from the public repository, pins its exact commit, generates Cargo sources from that commit's `Cargo.lock`, runs Flathub's manifest linter, and builds through `org.flatpak.Builder`. Flathub currently requires meaningful development history and normally rejects applications without a native graphical window or applications considered simple web wrappers. These scripts produce and validate the package, but do not guarantee catalog acceptance.

The repository must be public before Flathub can fetch its source. The project currently declares `LicenseRef-proprietary`; choose and add a distributable project license before submission if public redistribution is intended.

### AppImage

```sh
./build-appimage.sh
```

The script builds the Rust release, validates shared libraries and desktop metadata, creates a standards-compliant AppDir, downloads the official pinned `appimagetool` 1.9.0 for the current architecture when absent, and writes the AppImage under `dist/`. It uses `APPIMAGE_EXTRACT_AND_RUN=1`, so FUSE is not required on the build machine. Users normally need FUSE to mount an AppImage, but can also run it with `APPIMAGE_EXTRACT_AND_RUN=1`.

...
