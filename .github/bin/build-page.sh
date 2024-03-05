#!/usr/bin/env bash

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

docker run -v "${SCRIPT_DIR}/../..:/app" --entrypoint sh --workdir /app/docs node:20-buster build.sh
