#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <version> <package-kind>" >&2
  exit 1
fi

version="$1"
package_kind="$2"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "${script_dir}/.." && pwd)"
tag="v${version}"

if [[ -n "${SHOWEL_PKGREL_OVERRIDE:-}" ]]; then
  echo "${SHOWEL_PKGREL_OVERRIDE}"
  exit 0
fi

if ! git -C "${project_root}" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "1"
  exit 0
fi

if ! git -C "${project_root}" rev-parse --verify "${tag}^{commit}" >/dev/null 2>&1; then
  echo "1"
  exit 0
fi

case "${package_kind}" in
  aur-source)
    paths=(
      "packaging/aur/showel/PKGBUILD.in"
      "packaging/arch/showel.desktop"
      "scripts/render-aur-release-package.sh"
      "scripts/resolve-pkgrel.sh"
    )
    ;;
  aur-bin)
    paths=(
      ".github/workflows/linux.yml"
      "packaging/aur/showel-bin/PKGBUILD.in"
      "packaging/arch/showel.desktop"
      "scripts/render-aur-binary-package.sh"
      "scripts/resolve-pkgrel.sh"
    )
    ;;
  arch)
    paths=(
      "packaging/arch/PKGBUILD.in"
      "packaging/arch/showel.desktop"
      "scripts/render-arch-package.sh"
      "scripts/resolve-pkgrel.sh"
    )
    ;;
  *)
    echo "unknown package kind: ${package_kind}" >&2
    exit 1
    ;;
esac

commit_count="$(git -C "${project_root}" rev-list --count "${tag}..HEAD" -- "${paths[@]}")"
echo "$((commit_count + 1))"
