#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
readonly ROOT
readonly APP_ID="io.github.odioski.ViperPad"
readonly TEMPLATE="${ROOT}/packaging/flathub/${APP_ID}.yml.in"
readonly INPUT_DIR="${ROOT}/build/flatpak-input"
readonly MANIFEST="${INPUT_DIR}/${APP_ID}.yml"
readonly BUILD_DIR="${ROOT}/build/flatpak"
readonly REPO_DIR="${ROOT}/dist/flatpak-repo"
readonly BUNDLE="${ROOT}/dist/ViperPad.flatpak"

for command in git flatpak flatpak-builder; do
  command -v "${command}" >/dev/null || {
    echo "Missing required command: ${command}. Run ./install-deps.sh first." >&2
    exit 1
  }
done

if [[ -n $(git -C "${ROOT}" status --porcelain) ]]; then
  echo "The worktree must be clean so the pinned commit matches the files being built." >&2
  git -C "${ROOT}" status --short >&2
  exit 1
fi

COMMIT=$(git -C "${ROOT}" rev-parse --verify 'HEAD^{commit}')
SOURCE_URL="file://${ROOT}"

rm -rf "${INPUT_DIR}" "${BUILD_DIR}" "${REPO_DIR}" "${BUNDLE}"
mkdir -p "${INPUT_DIR}" "${ROOT}/dist"
"${ROOT}/scripts/generate-flatpak-sources.sh" \
  "${ROOT}/Cargo.lock" \
  "${INPUT_DIR}/cargo-sources.json"

sed \
  -e "s|@GIT_URL@|${SOURCE_URL}|g" \
  -e "s|@GIT_COMMIT@|${COMMIT}|g" \
  "${TEMPLATE}" > "${MANIFEST}"

echo "Building local commit ${COMMIT}"
flatpak-builder \
  --force-clean \
  --default-branch=stable \
  --repo="${REPO_DIR}" \
  "${BUILD_DIR}" \
  "${MANIFEST}"
flatpak build-bundle "${REPO_DIR}" "${BUNDLE}" "${APP_ID}" stable

echo "Built ${BUNDLE}"
