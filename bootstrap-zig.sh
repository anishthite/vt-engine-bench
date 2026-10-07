#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_DIR="${ROOT_DIR}/.context/zig"
VERSION="0.16.0"

mkdir -p "${OUT_DIR}"

os="$(uname -s)"
arch="$(uname -m)"

if [[ "${os}" != "Darwin" ]]; then
  echo "Unsupported OS: ${os}" >&2
  exit 1
fi

case "${arch}" in
  x86_64)
    tarball="zig-x86_64-macos-${VERSION}.tar.xz"
    shasum="0387557ed1877bc6a2e1802c8391953baddba76081876301c522f52977b52ba7"
    ;;
  arm64)
    tarball="zig-aarch64-macos-${VERSION}.tar.xz"
    shasum="b23d70deaa879b5c2d486ed3316f7eaa53e84acf6fc9cc747de152450d401489"
    ;;
  *)
    echo "Unsupported arch: ${arch}" >&2
    exit 1
    ;;
esac

url="https://ziglang.org/download/${VERSION}/${tarball}"

tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

archive="${tmp_dir}/${tarball}"

echo "Downloading ${url}"
curl -fsSL -o "${archive}" "${url}"

echo "${shasum}  ${archive}" | shasum -a 256 -c -

dest="${OUT_DIR}/${VERSION}"
rm -rf "${dest}"
mkdir -p "${dest}"

tar -xf "${archive}" -C "${dest}" --strip-components=1

ln -sfn "${dest}/zig" "${OUT_DIR}/zig"

echo "Installed Zig ${VERSION} to ${OUT_DIR}/zig"
