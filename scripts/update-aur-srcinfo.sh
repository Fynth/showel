#!/usr/bin/env bash

set -euo pipefail

pkg_dir="${1:-packaging/aur/showel-git}"

cd "${pkg_dir}"
makepkg --printsrcinfo > .SRCINFO
