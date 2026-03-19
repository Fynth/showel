#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <repo-dir> <repo-name>" >&2
  exit 1
fi

repo_dir="$1"
repo_name="$2"

shopt -s nullglob
packages=("${repo_dir}"/*.pkg.tar.*)

if [[ ${#packages[@]} -eq 0 ]]; then
  echo "no packages found in ${repo_dir}" >&2
  exit 1
fi

rm -f \
  "${repo_dir}/${repo_name}.db" \
  "${repo_dir}/${repo_name}.db.tar.gz" \
  "${repo_dir}/${repo_name}.files" \
  "${repo_dir}/${repo_name}.files.tar.gz"

repo-add "${repo_dir}/${repo_name}.db.tar.gz" "${packages[@]}"
ln -sf "${repo_name}.db.tar.gz" "${repo_dir}/${repo_name}.db"
ln -sf "${repo_name}.files.tar.gz" "${repo_dir}/${repo_name}.files"
