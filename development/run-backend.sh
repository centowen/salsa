#!/usr/bin/env bash
set -e

if [ ! -e "database.json" ]; then
    cp -r development/database.json database.json
fi

RUST_BACKTRACE=1 \
    RUST_LOG=Info \
    cargo run --bin backend
