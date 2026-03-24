#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 5 ]]; then
  echo "usage: $0 <version> <owner> <repo> <output-dir> <source-selector>" >&2
  exit 1
fi

version="$1"
owner="$2"
repo="$3"
output_dir="$4"
source_selector="$5"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "${script_dir}/.." && pwd)"
template="${project_root}/packaging/arch/PKGBUILD.in"
desktop_file="${project_root}/packaging/arch/showel.desktop"
pkgrel="$("${script_dir}/resolve-pkgrel.sh" "${version}" arch)"

mkdir -p "${output_dir}"
cp "${desktop_file}" "${output_dir}/showel.desktop"

desktop_sha="$(sha256sum "${desktop_file}" | awk '{print $1}')"

sed \
  -e "s/__PKGVER__/${version}/g" \
  -e "s/__PKGREL__/${pkgrel}/g" \
  -e "s/__OWNER__/${owner}/g" \
  -e "s/__REPO__/${repo}/g" \
  -e "s/__SOURCE_SELECTOR__/${source_selector}/g" \
  -e "s/__DESKTOP_SHA256__/${desktop_sha}/g" \
  "${template}" > "${output_dir}/PKGBUILD"
