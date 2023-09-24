#!/usr/bin/env bash
ROOT_PATH="$(dirname -- "${BASH_SOURCE[0]}")/.."
docker build -t salsa -f $ROOT_PATH/development/salsa.dockerfile $ROOT_PATH 
