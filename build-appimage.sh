#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
readonly ROOT
VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "${ROOT}/Cargo.toml" | head -n 1)
readonly VERSION
readonly TOOL_VERSION="1.9.0"

case "$(uname -m)" in
  x86_64) APPIMAGE_ARCH=x86_64 ;;
  aarch64|arm64) APPIMAGE_ARCH=aarch64 ;;
  i386|i486|i586|i686) APPIMAGE_ARCH=i686 ;;
  armv7l|armhf) APPIMAGE_ARCH=armhf ;;
  *) echo "Unsupported AppImage architecture: $(uname -m)" >&2; exit 1 ;;
esac
readonly APPIMAGE_ARCH
readonly TOOL_DIR="${ROOT}/.cache/appimagetool"
readonly APPIMAGETOOL="${TOOL_DIR}/appimagetool-${APPIMAGE_ARCH}.AppImage"
readonly APPDIR="${ROOT}/build/ViperPad.AppDir"
readonly OUTPUT="${ROOT}/dist/ViperPad-${VERSION}-${APPIMAGE_ARCH}.AppImage"

if [[ ! -x ${APPIMAGETOOL} ]]; then
  mkdir -p "${TOOL_DIR}"
  echo "Downloading official appimagetool ${TOOL_VERSION} for ${APPIMAGE_ARCH}..."
  curl --fail --location --retry 3 \
    "https://github.com/AppImage/appimagetool/releases/download/${TOOL_VERSION}/appimagetool-${APPIMAGE_ARCH}.AppImage" \
    --output "${APPIMAGETOOL}.part"
  mv "${APPIMAGETOOL}.part" "${APPIMAGETOOL}"
  chmod +x "${APPIMAGETOOL}"
fi

cargo build --manifest-path "${ROOT}/Cargo.toml" --release --locked
if ldd "${ROOT}/target/release/viperpad" | grep -q 'not found'; then
  echo "The release binary has unresolved shared libraries." >&2
  ldd "${ROOT}/target/release/viperpad" >&2
  exit 1
fi

rm -rf "${APPDIR}"
mkdir -p "${APPDIR}/usr/bin" "${APPDIR}/usr/share/applications" \
  "${APPDIR}/usr/share/icons/hicolor/512x512/apps" "${APPDIR}/usr/share/metainfo" "${ROOT}/dist"
install -Dm0755 "${ROOT}/target/release/viperpad" "${APPDIR}/usr/bin/viperpad"
install -Dm0755 "${ROOT}/packaging/appimage/AppRun" "${APPDIR}/AppRun"
install -Dm0644 "${ROOT}/packaging/linux/io.github.odioski.ViperPad.desktop" "${APPDIR}/usr/share/applications/io.github.odioski.ViperPad.desktop"
install -Dm0644 "${ROOT}/packaging/icons/io.github.odioski.ViperPad.png" "${APPDIR}/usr/share/icons/hicolor/512x512/apps/io.github.odioski.ViperPad.png"
install -Dm0644 "${ROOT}/packaging/linux/io.github.odioski.ViperPad.metainfo.xml" "${APPDIR}/usr/share/metainfo/io.github.odioski.ViperPad.appdata.xml"
ln -s usr/share/applications/io.github.odioski.ViperPad.desktop "${APPDIR}/io.github.odioski.ViperPad.desktop"
ln -s usr/share/icons/hicolor/512x512/apps/io.github.odioski.ViperPad.png "${APPDIR}/io.github.odioski.ViperPad.png"
ln -s io.github.odioski.ViperPad.png "${APPDIR}/.DirIcon"

desktop-file-validate "${APPDIR}/io.github.odioski.ViperPad.desktop"
if command -v appstreamcli >/dev/null; then
  appstreamcli validate --no-net "${APPDIR}/usr/share/metainfo/io.github.odioski.ViperPad.appdata.xml"
fi

rm -f "${OUTPUT}"
env APPIMAGE_EXTRACT_AND_RUN=1 ARCH="${APPIMAGE_ARCH}" VERSION="${VERSION}" \
  "${APPIMAGETOOL}" --no-appstream "${APPDIR}" "${OUTPUT}"
chmod +x "${OUTPUT}"
echo "Built ${OUTPUT}"
echo "Base-system runtime libraries: glibc, libm, libgcc_s, and the ELF loader."
