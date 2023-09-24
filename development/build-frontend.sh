#!/usr/bin/env bash
cd frontend && mkdir -p ../artifacts/frontend/salsa && trunk build --public-url salsa/ --release -d ../artifacts/frontend/salsa && cd ..
export FRONTEND_PATH=$(pwd)/artifacts/frontend
