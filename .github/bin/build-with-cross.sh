#!/bin/bash
set -eux
DIR="$(cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd)"

# install cross if it doesn't exist
if ! command -v cross &> /dev/null; then
    cargo install cross
fi

cross build --release --target $1

ARTIFACT_DIR="$DIR/../../target/dev-scope-$1"
rm -rf $ARTIFACT_DIR || true
mkdir $ARTIFACT_DIR
cp $DIR/../../LICENSE $ARTIFACT_DIR
cp $DIR/../../README.md $ARTIFACT_DIR
cp $DIR/../../scope/CHANGELOG.md $ARTIFACT_DIR
cp $DIR/../../target/$1/release/scope $ARTIFACT_DIR
cp $DIR/../../target/$1/release/scope-intercept $ARTIFACT_DIR

pushd $DIR/../../target
rm dev-scope-$1.tar.xz || true
tar cfJ dev-scope-$1.tar.xz dev-scope-$1

