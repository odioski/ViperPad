#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
readonly ROOT
readonly APP_ID="io.github.odioski.ViperPad"
readonly SOURCE_URL="https://github.com/odioski/ViperPad.git"
readonly TEMPLATE="${ROOT}/packaging/flathub/${APP_ID}.yml.in"
readonly OUTPUT_DIR="${ROOT}/build/flathub"
readonly MANIFEST_NAME="${APP_ID}.yml"
readonly MANIFEST="${OUTPUT_DIR}/${MANIFEST_NAME}"
readonly LOCK_FILE="${OUTPUT_DIR}/Cargo.lock"
REF=${1:-main}

for command in git flatpak; do
  command -v "${command}" >/dev/null || {
    echo "Missing required command: ${command}. Run ./install-deps.sh first." >&2
    exit 1
  }
done

if [[ -n $(git -C "${ROOT}" status --porcelain) ]]; then
  echo "The worktree must be clean before preparing a Flathub submission." >&2
  git -C "${ROOT}" status --short >&2
  exit 1
fi

REMOTE_REFS=$(git ls-remote --exit-code "${SOURCE_URL}" \
  "${REF}" "refs/heads/${REF}" "refs/tags/${REF}" "refs/tags/${REF}^{}") || {
  echo "Remote branch or tag not found: ${REF}" >&2
  exit 1
}

COMMIT=$(awk -v ref="${REF}" '
  $2 == "refs/tags/" ref "^{}" { peeled = $1 }
  $2 == "refs/heads/" ref { branch = $1 }
  $2 == "refs/tags/" ref { tag = $1 }
  $2 == ref { exact = $1 }
  END {
    if (peeled) print peeled
    else if (branch) print branch
    else if (tag) print tag
    else print exact
  }
' <<< "${REMOTE_REFS}")

if [[ ! ${COMMIT} =~ ^[0-9a-f]{40}$ ]]; then
  echo "Could not resolve '${REF}' to a remote Git commit." >&2
  exit 1
fi

rm -rf "${OUTPUT_DIR}"
mkdir -p "${OUTPUT_DIR}"
git -C "${ROOT}" fetch --quiet --no-tags "${SOURCE_URL}" "${COMMIT}"
git -C "${ROOT}" show "${COMMIT}:Cargo.lock" > "${LOCK_FILE}"
"${ROOT}/scripts/generate-flatpak-sources.sh" \
  "${LOCK_FILE}" \
  "${OUTPUT_DIR}/cargo-sources.json"
rm "${LOCK_FILE}"

sed \
  -e "s|@GIT_URL@|${SOURCE_URL}|g" \
  -e "s|@GIT_COMMIT@|${COMMIT}|g" \
  "${TEMPLATE}" > "${MANIFEST}"
cp "${ROOT}/packaging/flathub/flathub.json" "${OUTPUT_DIR}/flathub.json"

echo "Prepared ${REF} at commit ${COMMIT}"
(
  cd "${OUTPUT_DIR}"
  flatpak run \
    --filesystem="${OUTPUT_DIR}" \
    --cwd="${OUTPUT_DIR}" \
    --command=flatpak-builder-lint \
    org.flatpak.Builder manifest "${MANIFEST_NAME}"
  flatpak run \
    --filesystem="${OUTPUT_DIR}" \
    --cwd="${OUTPUT_DIR}" \
    --command=flathub-build \
    org.flatpak.Builder --install "${MANIFEST_NAME}"
)
