#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 4 && $# -ne 5 ]]; then
  echo "usage: $0 <version> <owner> <repo> <output-dir> [source-sha256]" >&2
  exit 1
fi

version="$1"
owner="$2"
repo="$3"
output_dir="$4"
source_sha="${5:-}"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "${script_dir}/.." && pwd)"
template="${project_root}/packaging/aur/shovel-bin/PKGBUILD.in"
pkgrel="$("${script_dir}/resolve-pkgrel.sh" "${version}" aur-bin)"
tag_name="$("${script_dir}/resolve-release-tag.sh" "${version}")"
wait_seconds="${SHOVEL_BINARY_ASSET_WAIT_SECONDS:-900}"
wait_interval="${SHOVEL_BINARY_ASSET_WAIT_INTERVAL:-15}"

if [[ -z "${source_sha}" ]]; then
  source_url="https://github.com/${owner}/${repo}/releases/download/${tag_name}/shovel-linux-x86_64.tar.gz"
  archive_file="$(mktemp)"
  trap 'rm -f "${archive_file}"' EXIT

  deadline="$(( $(date +%s) + wait_seconds ))"
  while ! curl -fsSL "${source_url}" -o "${archive_file}"; do
    if (( $(date +%s) >= deadline )); then
      echo "failed to download ${source_url} after waiting ${wait_seconds}s" >&2
      exit 1
    fi

    echo "waiting for release asset ${source_url}" >&2
    sleep "${wait_interval}"
  done

  source_sha="$(sha256sum "${archive_file}" | awk '{print $1}')"
fi

mkdir -p "${output_dir}"

sed \
  -e "s/__PKGVER__/${version}/g" \
  -e "s/__PKGREL__/${pkgrel}/g" \
  -e "s/__TAG_NAME__/${tag_name}/g" \
  -e "s/__OWNER__/${owner}/g" \
  -e "s/__REPO__/${repo}/g" \
  -e "s/__SOURCE_SHA256__/${source_sha}/g" \
  "${template}" > "${output_dir}/PKGBUILD"
