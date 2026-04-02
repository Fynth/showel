#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: $0 <version> <repo-dir> <repo-url>" >&2
  exit 1
fi

version="$1"
repo_dir="$2"
repo_url="$3"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "${script_dir}/.." && pwd)"
app_id="dev.showel.app"
manifest_rel="packaging/flatpak/${app_id}.yml"
build_root="$(mktemp -d)"
source_root="${build_root}/src"
state_root="${build_root}/build"

mkdir -p "${source_root}"
rm -rf "${repo_dir}"
mkdir -p "${repo_dir}"

cleanup() {
  rm -rf "${build_root}"
}
trap cleanup EXIT

git -C "${project_root}" archive --format=tar HEAD | tar -xf - -C "${source_root}"
mkdir -p "${source_root}/.cargo"

(
  cd "${source_root}"
  cargo vendor --locked --versioned-dirs vendor > .cargo/config.toml
)

flatpak-builder \
  --force-clean \
  --repo="${repo_dir}" \
  "${state_root}" \
  "${source_root}/${manifest_rel}"

flatpak build-update-repo \
  --generate-static-deltas \
  --title="Showel Flatpak Repository" \
  "${repo_dir}"

cat > "${repo_dir}/showel.flatpakrepo" <<EOF
[Flatpak Repo]
Title=Showel Flatpak Repository
Comment=Flatpak repository for Showel ${version}
Url=${repo_url}
Homepage=https://github.com/Fynth/showel
Icon=https://raw.githubusercontent.com/Fynth/showel/refs/heads/main/app/assets/icon.png
DefaultBranch=stable
GPGVerify=false
EOF

cat > "${repo_dir}/index.html" <<EOF
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Showel Flatpak Repository</title>
</head>
<body>
  <h1>Showel Flatpak Repository</h1>
  <p>Add the repository:</p>
  <pre><code>flatpak remote-add --user --if-not-exists showel-flatpak ${repo_url}/showel.flatpakrepo
flatpak install --user showel-flatpak ${app_id}</code></pre>
</body>
</html>
EOF
