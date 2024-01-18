#!/usr/bin/env bash
set -euxo pipefail

export VERSION="${1:-'0.1.1-SNAPSHOT'}"

cross build --target aarch64-unknown-linux-gnu --release
cross build --target x86_64-unknown-linux-gnu --release

echo "Built artifacts for amd64"
file target/x86_64-unknown-linux-gnu/release/scope
file target/x86_64-unknown-linux-gnu/release/scope-intercept

echo "Built artifacts for aarch64"
file target/aarch64-unknown-linux-gnu/release/scope
file target/aarch64-unknown-linux-gnu/release/scope-intercept

rm -rf target/x86_64-unknown-linux-gnu/artifact || true
mkdir -p target/x86_64-unknown-linux-gnu/artifact

cp target/x86_64-unknown-linux-gnu/release/scope target/x86_64-unknown-linux-gnu/artifact
cp target/x86_64-unknown-linux-gnu/release/scope-intercept target/x86_64-unknown-linux-gnu/artifact

rm -rf target/aarch64-unknown-linux-gnu/artifact || true
mkdir -p target/aarch64-unknown-linux-gnu/artifact
cp target/aarch64-unknown-linux-gnu/release/scope target/aarch64-unknown-linux-gnu/artifact
cp target/aarch64-unknown-linux-gnu/release/scope-intercept target/aarch64-unknown-linux-gnu/artifact
