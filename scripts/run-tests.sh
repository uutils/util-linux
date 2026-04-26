#!/bin/bash

set -euxo pipefail

cd "$(dirname "$0")/../"
PROJECT_DIR="$(pwd)"

cargo build --release

./scripts/gen-test-helper.sh

# Clear stale diff files from any previous run
rm -rf "$PROJECT_DIR/.test-helpers/tests/diff"

# Run GNU tests; allow non-zero exit (known failures exist)
set +e
"$GNU_PROJECT_DIR/tests/run.sh" \
    --builddir="$PROJECT_DIR/.test-helpers" \
    "$@" \
    cal dmesg hexdump lscpu lslocks lsmem
GNU_TEST_EXIT=$?
set -e

[[ $GNU_TEST_EXIT -ne 0 ]] && echo "GNU test runner exited with code $GNU_TEST_EXIT"

./scripts/check-new-gnu-failures.sh
