#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <version> <output-dir>" >&2
  exit 1
fi

version="$1"
output_dir="$2"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "${script_dir}/.." && pwd)"
app_id="dev.shovel.app"
manifest_rel="packaging/flatpak/${app_id}.yml"
runtime_repo="https://flathub.org/repo/flathub.flatpakrepo"
bundle_path="${output_dir}/shovel-linux-x86_64.flatpak"
build_root="$(mktemp -d)"
source_root="${build_root}/src"
repo_root="${build_root}/repo"
state_root="${build_root}/build"

mkdir -p "${source_root}"
mkdir -p "${output_dir}"

git -C "${project_root}" archive --format=tar HEAD | tar -xf - -C "${source_root}"
mkdir -p "${source_root}/.cargo"

(
  cd "${source_root}"
  cargo vendor --locked --versioned-dirs vendor > .cargo/config.toml
)

flatpak-builder \
  --force-clean \
  --default-branch=stable \
  --repo="${repo_root}" \
  "${state_root}" \
  "${source_root}/${manifest_rel}"

flatpak build-bundle \
  --runtime-repo="${runtime_repo}" \
  "${repo_root}" \
  "${bundle_path}" \
  "${app_id}" \
  stable

echo "${bundle_path}"
