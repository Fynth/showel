#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <version>" >&2
  exit 1
fi

version="$1"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "${script_dir}/.." && pwd)"

if [[ -n "${SHOVEL_TAG_OVERRIDE:-}" ]]; then
  echo "${SHOVEL_TAG_OVERRIDE}"
  exit 0
fi

if git -C "${project_root}" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  if git -C "${project_root}" rev-parse --verify "${version}^{commit}" >/dev/null 2>&1; then
    echo "${version}"
    exit 0
  fi

  if git -C "${project_root}" rev-parse --verify "v${version}^{commit}" >/dev/null 2>&1; then
    echo "v${version}"
    exit 0
  fi
fi

echo "${version}"
