#!/bin/bash

set -euxo pipefail

cd "$(dirname "$0")/../"
PROJECT_DIR="$(pwd)"

cargo build --release

./scripts/gen-test-helper.sh

"$GNU_PROJECT_DIR/tests/run.sh" \
    --builddir="$PROJECT_DIR/.test-helpers" \
    "$@" \
    cal dmesg hexdump lscpu lslocks lsmem
