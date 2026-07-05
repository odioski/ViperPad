#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
readonly ROOT
readonly TOOLS_COMMIT="737c0085912f9f7dabf9341d4608e2a77a51a73a"
readonly TOOL_DIR="${ROOT}/.cache/flatpak-cargo-generator"
readonly GENERATOR="${TOOL_DIR}/flatpak-cargo-generator.py"
readonly VENV="${ROOT}/.venv"
LOCK_FILE=${1:-"${ROOT}/Cargo.lock"}
OUTPUT=${2:-"${ROOT}/build/cargo-sources.json"}

if [[ ! -x ${VENV}/bin/python ]]; then
  echo "Missing ${VENV}. Run ./install-deps.sh first." >&2
  exit 1
fi

mkdir -p "${TOOL_DIR}"
mkdir -p "$(dirname -- "${OUTPUT}")"
if [[ ! -f ${GENERATOR} ]]; then
  curl --fail --location --retry 3 \
    "https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/${TOOLS_COMMIT}/cargo/flatpak-cargo-generator.py" \
    --output "${GENERATOR}"
fi

"${VENV}/bin/python" "${GENERATOR}" "${LOCK_FILE}" --output "${OUTPUT}"
echo "Generated ${OUTPUT}"
