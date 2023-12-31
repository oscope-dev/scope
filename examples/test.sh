#!/usr/bin/env -S cargo run --bin pity-intercept -- --extra-config examples bash

>&2 echo "error 1!"
sleep 1
echo "hello world"
exit 1