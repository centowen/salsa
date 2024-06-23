#!/usr/bin/env bash
set -e

if [ ! -e "database.json" ]; then
    cp -r development/database.json database.json
fi
FRONTEND_PATH=backend/frontend RUST_LOG=Info cargo run --package backend --bin backend 
