#!/usr/bin/env bash
set -e

RUST_BACKTRACE=1 \
    RUST_LOG=Info \
    cargo run --bin backend
