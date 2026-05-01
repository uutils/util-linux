#!/bin/bash

set -euxo pipefail

cd "$(dirname "$0")/../"
PROJECT_DIR="$(pwd)"

cargo build --release

./scripts/gen-test-helper.sh

mapfile -t UTILS < "$PROJECT_DIR/.test-helpers/utils.list"

if [[ ${#UTILS[@]} -eq 0 ]]; then
    echo "ERROR: .test-helpers/utils.list is empty; nothing to test." >&2
    exit 1
fi

# Clear stale diff files from any previous run
rm -rf "$PROJECT_DIR/.test-helpers/tests/diff"

# Run GNU tests; allow non-zero exit (known failures exist)
set +e
"$GNU_PROJECT_DIR/tests/run.sh" \
    --builddir="$PROJECT_DIR/.test-helpers" \
    "$@" \
    "${UTILS[@]}"
GNU_TEST_EXIT=$?
set -e

[[ $GNU_TEST_EXIT -ne 0 ]] && echo "GNU test runner exited with code $GNU_TEST_EXIT"

./scripts/check-new-gnu-failures.sh
