#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: $0 <package-name> <package-dir> <version>" >&2
  exit 1
fi

package_name="$1"
package_dir="$2"
version="$3"
aur_repo="${RUNNER_TEMP:-/tmp}/aur-${package_name}"

with_retry() {
  local attempts="${SHOVEL_SSH_RETRY_ATTEMPTS:-5}"
  local delay_seconds="${SHOVEL_SSH_RETRY_DELAY_SECONDS:-5}"
  local attempt=1
  local exit_code=0

  while true; do
    if "$@"; then
      return 0
    fi
    exit_code=$?

    if (( attempt >= attempts )); then
      return "${exit_code}"
    fi

    echo "command failed with exit code ${exit_code}; retry ${attempt}/${attempts} in ${delay_seconds}s: $*" >&2
    sleep "${delay_seconds}"
    attempt=$((attempt + 1))
  done
}

rm -rf "${aur_repo}"
with_retry git clone "ssh://aur@aur.archlinux.org/${package_name}.git" "${aur_repo}"
cd "${aur_repo}"

if ! git rev-parse --verify HEAD >/dev/null 2>&1; then
  git checkout --orphan master
fi

cp "${package_dir}/PKGBUILD" "${aur_repo}/PKGBUILD"
cp "${package_dir}/.SRCINFO" "${aur_repo}/.SRCINFO"

git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"

if [[ -z "$(git status --porcelain)" ]]; then
  echo "${package_name} is already up to date"
  exit 0
fi

git add PKGBUILD .SRCINFO
git commit -m "${package_name} ${version}"
with_retry git push origin HEAD:master
