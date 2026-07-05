# Changelog

## 2026-06-23

- Rebranded the project from **Snake-IDE** to **ViperPad** across the app UI, docs, package metadata, and Linux packaging assets.
- Removed the tracked Flatpak cargo source manifest so it is generated on demand instead, and deleted the stale GitHub release/tag with old-name downloadable binaries.
- Added a **Clear** button next to the address field and hid it on narrow screens alongside the copy button and status text.
- Changed the address field to show the loaded source as placeholder text instead of keeping the viewer URL in the input after opening a file.
- Added source formatting helpers so loaded local files and remote URLs display a cleaner label in the address field.
- Updated local file open/upload flow to track the current source, clear the input after upload, and keep the opened file name visible when selected from the picker.
- Changed **Copy** to prefer the typed value, then the current source, then the generated viewer URL.
