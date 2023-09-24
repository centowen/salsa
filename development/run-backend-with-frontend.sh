#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(dirname "$(readlink -f "$0")")"
ENV_FILE="$SCRIPT_DIR/env.sh"
if [ -f "$ENV_FILE" ]; then
    source "$ENV_FILE"
fi

source "$SCRIPT_DIR/build-frontend.sh"
$SCRIPT_DIR/run-backend.sh
