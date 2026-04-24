#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <repo-root> <repo-url>" >&2
  exit 1
fi

repo_root="$1"
repo_url="$2"
dist_dir="${repo_root}/dists/stable/main/binary-amd64"
pool_dir="${repo_root}/pool/main/s/shovel"

mkdir -p "${dist_dir}"
mkdir -p "${pool_dir}"

shopt -s nullglob
packages=("${pool_dir}"/*.deb)
if [[ ${#packages[@]} -eq 0 ]]; then
  echo "no deb packages found in ${pool_dir}" >&2
  exit 1
fi

(
  cd "${repo_root}"
  dpkg-scanpackages --arch amd64 pool /dev/null > "dists/stable/main/binary-amd64/Packages"
)
gzip -kf "${dist_dir}/Packages"

packages_rel="main/binary-amd64/Packages"
packages_gz_rel="main/binary-amd64/Packages.gz"
packages_size="$(wc -c < "${dist_dir}/Packages")"
packages_gz_size="$(wc -c < "${dist_dir}/Packages.gz")"
packages_md5="$(md5sum "${dist_dir}/Packages" | awk '{print $1}')"
packages_gz_md5="$(md5sum "${dist_dir}/Packages.gz" | awk '{print $1}')"
packages_sha256="$(sha256sum "${dist_dir}/Packages" | awk '{print $1}')"
packages_gz_sha256="$(sha256sum "${dist_dir}/Packages.gz" | awk '{print $1}')"

cat > "${repo_root}/dists/stable/Release" <<EOF
Origin: Shovel
Label: Shovel
Suite: stable
Codename: stable
Architectures: amd64
Components: main
Date: $(LC_ALL=C date -Ru)
MD5Sum:
 ${packages_md5} ${packages_size} ${packages_rel}
 ${packages_gz_md5} ${packages_gz_size} ${packages_gz_rel}
SHA256:
 ${packages_sha256} ${packages_size} ${packages_rel}
 ${packages_gz_sha256} ${packages_gz_size} ${packages_gz_rel}
EOF

cat > "${repo_root}/index.html" <<EOF
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Shovel APT Repository</title>
</head>
<body>
  <h1>Shovel APT Repository</h1>
  <p>Add this repository:</p>
  <pre><code>echo "deb [arch=amd64 trusted=yes] ${repo_url} stable main" | sudo tee /etc/apt/sources.list.d/shovel.list
sudo apt update
sudo apt install shovel</code></pre>
</body>
</html>
EOF
