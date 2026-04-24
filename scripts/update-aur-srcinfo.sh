#!/usr/bin/env bash

set -euo pipefail

pkg_dir="${1:-packaging/aur/shovel-git}"

cd "${pkg_dir}"
makepkg --printsrcinfo > .SRCINFO
