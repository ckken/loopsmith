#!/usr/bin/env bash
set -euo pipefail

target="${1:-}"
version="${2:-}"
dist_dir="${DIST_DIR:-dist}"

if [[ -z "${target}" ]]; then
  target="$(rustc -vV | sed -n 's/^host: //p')"
fi

if [[ -z "${version}" ]]; then
  version="$(awk -F '"' '/^version = / { print $2; exit }' Cargo.toml)"
fi

if [[ -z "${target}" || -z "${version}" ]]; then
  echo "usage: scripts/package-release.sh [target] [version]" >&2
  exit 2
fi

tag="v${version}"
package="loopsmith-${tag}-${target}"
binary="loopsmith"

if [[ "${target}" == *"windows"* ]]; then
  binary="loopsmith.exe"
fi

cargo build --release --locked --target "${target}"

rm -rf "${dist_dir:?}/${package}" \
  "${dist_dir}/${package}.tar.gz" \
  "${dist_dir}/${package}.zip" \
  "${dist_dir}/${package}.tar.gz.sha256" \
  "${dist_dir}/${package}.zip.sha256"

mkdir -p "${dist_dir}/${package}/docs"
cp "target/${target}/release/${binary}" "${dist_dir}/${package}/"
cp README.md "${dist_dir}/${package}/"
cp docs/loopsmith-best-practices.md "${dist_dir}/${package}/docs/"

if [[ "${target}" == *"windows"* ]]; then
  archive="${package}.zip"
  if command -v 7z >/dev/null 2>&1; then
    (cd "${dist_dir}" && 7z a -tzip "${archive}" "${package}" >/dev/null)
  elif command -v powershell.exe >/dev/null 2>&1; then
    (cd "${dist_dir}" && powershell.exe -NoProfile -Command "Compress-Archive -Path '${package}' -DestinationPath '${archive}' -Force")
  else
    echo "zip packaging requires 7z or powershell.exe on Windows" >&2
    exit 1
  fi
else
  archive="${package}.tar.gz"
  tar -C "${dist_dir}" -czf "${dist_dir}/${archive}" "${package}"
fi

if command -v shasum >/dev/null 2>&1; then
  (cd "${dist_dir}" && shasum -a 256 "${archive}" > "${archive}.sha256")
elif command -v sha256sum >/dev/null 2>&1; then
  (cd "${dist_dir}" && sha256sum "${archive}" > "${archive}.sha256")
else
  echo "checksum generation requires shasum or sha256sum" >&2
  exit 1
fi

echo "${dist_dir}/${archive}"
echo "${dist_dir}/${archive}.sha256"
