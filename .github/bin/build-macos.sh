#!/usr/bin/env bash
set -euxo pipefail

VERSION="${1:-'0.1.1-SNAPSHOT'}"

cargo build --release --target aarch64-apple-darwin --config package.version=\"${VERSION}\"
cargo build --release --target x86_64-apple-darwin --config package.version=\"${VERSION}\"
rm -rf target/universal-apple-darwin/release || true
mkdir -p target/universal-apple-darwin/release

lipo -create -output target/universal-apple-darwin/release/pity \
  target/x86_64-apple-darwin/release/pity \
  target/aarch64-apple-darwin/release/pity

lipo -create -output target/universal-apple-darwin/release/pity-intercept \
  target/x86_64-apple-darwin/release/pity-intercept \
  target/aarch64-apple-darwin/release/pity-intercept

echo "Built a multi-arch binary at target/universal-apple-darwin/release"
file target/universal-apple-darwin/release/pity
file target/universal-apple-darwin/release/pity-intercept

target/universal-apple-darwin/release/pity --help
target/universal-apple-darwin/release/pity-intercept --help

