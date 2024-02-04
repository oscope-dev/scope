#!/usr/bin/env bash
set -euxo pipefail

cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin
rm -rf target/universal-apple-darwin/release || true
mkdir -p target/universal-apple-darwin/release

lipo -create -output target/universal-apple-darwin/release/scope \
  target/x86_64-apple-darwin/release/scope \
  target/aarch64-apple-darwin/release/scope

lipo -create -output target/universal-apple-darwin/release/scope-intercept \
  target/x86_64-apple-darwin/release/scope-intercept \
  target/aarch64-apple-darwin/release/scope-intercept

echo "Built a multi-arch binary at target/universal-apple-darwin/release"
file target/universal-apple-darwin/release/scope
file target/universal-apple-darwin/release/scope-intercept

target/universal-apple-darwin/release/scope --help
target/universal-apple-darwin/release/scope-intercept --help

