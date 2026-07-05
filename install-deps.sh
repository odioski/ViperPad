#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
readonly ROOT
readonly FLATPAK_BRANCH="25.08"
readonly FLATHUB_URL="https://dl.flathub.org/repo/flathub.flatpakrepo"
readonly VENV="${ROOT}/.venv"
VENV_ONLY=false

if [[ ${1:-} == "--venv-only" ]]; then
  VENV_ONLY=true
elif (( $# > 0 )); then
  echo "Usage: $0 [--venv-only]" >&2
  exit 1
fi

if [[ ${VENV_ONLY} == false ]]; then
  if [[ ${EUID} -eq 0 ]]; then
    SUDO=()
  else
    command -v sudo >/dev/null || { echo "sudo is required to install system packages." >&2; exit 1; }
    SUDO=(sudo)
  fi

  if command -v dnf >/dev/null; then
    FEDORA_NODE_PACKAGES=()
    if ! command -v node >/dev/null || ! command -v npm >/dev/null; then
      FEDORA_NODE_PACKAGES=(nodejs24 nodejs24-npm)
    fi
    "${SUDO[@]}" dnf install -y rust cargo gcc binutils git curl file flatpak flatpak-builder python3 python3-pip desktop-file-utils appstream patchelf squashfs-tools "${FEDORA_NODE_PACKAGES[@]}"
  elif command -v apt-get >/dev/null; then
    "${SUDO[@]}" apt-get update
    "${SUDO[@]}" apt-get install -y rustc cargo build-essential binutils git curl file flatpak flatpak-builder python3 python3-pip python3-venv nodejs npm desktop-file-utils appstream patchelf squashfs-tools
  elif command -v pacman >/dev/null; then
    "${SUDO[@]}" pacman -S --needed --noconfirm rust cargo base-devel binutils git curl file flatpak flatpak-builder python python-pip nodejs npm desktop-file-utils appstream patchelf squashfs-tools
  elif command -v zypper >/dev/null; then
    "${SUDO[@]}" zypper --non-interactive install rust cargo gcc gcc-c++ binutils git curl file flatpak flatpak-builder python3 python3-pip nodejs npm desktop-file-utils AppStream patchelf squashfs
  else
    echo "Unsupported package manager. Install the tools listed in README.md manually." >&2
    exit 1
  fi

  flatpak remote-add --user --if-not-exists flathub "${FLATHUB_URL}"
  flatpak install --user -y flathub \
    "org.freedesktop.Platform//${FLATPAK_BRANCH}" \
    "org.freedesktop.Sdk//${FLATPAK_BRANCH}" \
    "org.freedesktop.Sdk.Extension.rust-stable//${FLATPAK_BRANCH}" \
    org.flatpak.Builder
fi

python3 -m venv "${VENV}"
"${VENV}/bin/python" -m pip install --upgrade pip
"${VENV}/bin/python" -m pip install \
  'aiohttp>=3.9.5,<4' \
  'PyYAML>=6.0.2,<7' \
  'tomlkit>=0.13.3,<1'

echo "ViperPad build dependencies are installed."
