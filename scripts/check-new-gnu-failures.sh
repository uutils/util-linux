#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/../"
PROJECT_DIR="$(pwd)"
DIFF_DIR="$PROJECT_DIR/.test-helpers/tests/diff"
BASELINE="$PROJECT_DIR/.reference/gnu-test-failures.txt"
UPDATE_BASELINE=false
EMPTY_BASELINE=false

for arg in "$@"; do
    case "$arg" in
        --update-baseline) UPDATE_BASELINE=true ;;
        *) echo "Unknown argument: $arg" >&2; exit 2 ;;
    esac
done

# Collect current failures (exclude .err companion files)
current_tmp=$(mktemp)
trap 'rm -f "$current_tmp"' EXIT
if [[ -d "$DIFF_DIR" ]]; then
    find "$DIFF_DIR" -mindepth 2 -maxdepth 2 -type f ! -name "*.err" -print0 \
        | sort -z \
        | while IFS= read -r -d '' path; do
            echo "${path#"$DIFF_DIR/"}"
          done > "$current_tmp"
fi

# --update-baseline mode
if [[ "$UPDATE_BASELINE" == true ]]; then
    {
        echo "# Known GNU test failures - format: util/testname"
        echo "# Update with: ./scripts/check-new-gnu-failures.sh --update-baseline"
        cat "$current_tmp"
    } > "$BASELINE"
    count=$(wc -l < "$current_tmp")
    echo "Baseline updated: $BASELINE ($count failures recorded)"
    exit 0
fi

# Load baseline (strip comments and blank lines)
baseline_tmp=$(mktemp)
trap 'rm -f "$current_tmp" "$baseline_tmp"' EXIT
if [[ -f "$BASELINE" ]]; then
    grep -v '^\s*#' "$BASELINE" | grep -v '^\s*$' | sort > "$baseline_tmp"
else
    echo "WARNING: Baseline not found: $BASELINE" >&2
    echo "  Run with --update-baseline to create it." >&2
    touch "$baseline_tmp"
    EMPTY_BASELINE=true
fi

# comm: -13 = lines only in current (new failures), -23 = lines only in baseline (fixed)
new_failures=$(comm -13 "$baseline_tmp" "$current_tmp")
fixed_tests=$(comm -23 "$baseline_tmp" "$current_tmp")

# Report
echo "--- GNU test failure summary ---"
echo "Baseline failures: $(wc -l < "$baseline_tmp")"
echo "Current failures:  $(wc -l < "$current_tmp")"
echo ""

if [[ "$EMPTY_BASELINE" == true ]]; then
    mkdir -p "$(dirname "$BASELINE")"
    {
        echo "# Known GNU test failures - format: util/testname"
        echo "# Update with: ./scripts/check-new-gnu-failures.sh --update-baseline"
        cat "$current_tmp"
    } > "$BASELINE"
    count=$(wc -l < "$current_tmp")
    echo "Initial baseline created: $BASELINE ($count failures recorded)"
    exit 0
fi

if [[ -n "$fixed_tests" ]]; then
    echo "Tests newly FIXED:"
    awk '{print " [FIXED] " $0}' <<< "$fixed_tests"
    echo ""
fi

if [[ -n "$new_failures" ]]; then
    echo "Tests newly FAILING:"
    awk '{print " [NEW FAILURE] " $0}' <<< "$new_failures"
    echo ""
    count=$(wc -l <<< "$new_failures")
    echo "ERROR: $count new test failure(s) detected." >&2
    echo "  Fix the regression, or if intentional, update the baseline:" >&2
    echo "    ./scripts/check-new-gnu-failures.sh --update-baseline" >&2
    exit 1
fi

echo "No new failures. All current failures are known."
if [[ -n "$fixed_tests" ]]; then
    echo "Consider updating the baseline to remove fixed tests:"
    echo "  ./scripts/check-new-gnu-failures.sh --update-baseline"
fi
