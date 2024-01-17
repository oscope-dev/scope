#!/usr/bin/env bash
set -euxo pipefail

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
TARGET_DIR="${SCRIPT_DIR}/../../target/"
PKG_DIR="${SCRIPT_DIR}/../../pkg/"
VERSION="${1:-'0.1.1-SNAPSHOT'}"

cp "${PKG_DIR}/nfpm-amd64.tmpl.yaml" "${TARGET_DIR}/nfpm-amd64.yaml"
echo "" >> "${TARGET_DIR}/nfpm-amd64.yaml"
echo "version: ${VERSION}" >> "${TARGET_DIR}/nfpm-amd64.yaml"
cp "${PKG_DIR}/nfpm-aarch64.tmpl.yaml" "${TARGET_DIR}/nfpm-aarch64.yaml"
echo "" >> "${TARGET_DIR}/nfpm-aarch64.yaml"
echo "version: ${VERSION}" >> "${TARGET_DIR}/nfpm-aarch64.yaml"

rm -rf "${TARGET_DIR}/upload" || true
mkdir "${TARGET_DIR}/upload"

docker run --rm -v "${TARGET_DIR}:/tmp" -w /tmp goreleaser/nfpm \
  package --config "/tmp/nfpm-amd64.yaml" \
  --packager rpm \
  --target "/tmp/upload"

docker run --rm -v "${TARGET_DIR}:/tmp" -w /tmp goreleaser/nfpm \
  package --config "/tmp/nfpm-amd64.yaml" \
  --packager deb \
  --target "/tmp/upload"

docker run --rm -v "${TARGET_DIR}:/tmp" -w /tmp goreleaser/nfpm \
  package --config "/tmp/nfpm-aarch64.yaml" \
  --packager rpm \
  --target "/tmp/upload"

docker run --rm -v "${TARGET_DIR}:/tmp" -w /tmp goreleaser/nfpm \
  package --config "/tmp/nfpm-aarch64.yaml" \
  --packager deb \
  --target "/tmp/upload"

tar -C "${TARGET_DIR}/x86_64-unknown-linux-gnu/artifact/" \
  -cvzf "${TARGET_DIR}/upload/scope-${{ inputs.version }}-x86_64-unknown-linux-gnu.tgz" \
  scope scope-intercept
tar -C "${TARGET_DIR}/aarch64-unknown-linux-gnu/artifact/" \
  -cvzf "${TARGET_DIR}/upload/scope-${{ inputs.version }}-aarch64-unknown-linux-gnu.tgz" \
  scope scope-intercept