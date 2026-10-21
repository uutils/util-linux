#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/../"
PROJECT_DIR="$(pwd)"
DIFF_DIR="$PROJECT_DIR/.test-helpers/tests/diff"
BASELINE="$PROJECT_DIR/tests/gnu-test-failures.txt"
UPDATE_BASELINE=false

for arg in "$@"; do
    case "$arg" in
        --update-baseline) UPDATE_BASELINE=true ;;
        *) echo "Unknown argument: $arg" >&2; exit 2 ;;
    esac
done

# run-tests.sh deletes DIFF_DIR before running the GNU tests, and the GNU
# harness (ts_gen_diff in tests/functions.sh) creates DIFF_DIR/<component>
# whenever it compares a test's output, deleting only the diff files of
# tests that pass. So no component directory means that no test produced
# results: run.sh failed to start, died early, or skipped every test.
# Without this check, such a run would have no failures, so every known
# failure would be reported as fixed and the check would pass (or
# --update-baseline would record an empty baseline).
if [[ ! -d "$DIFF_DIR" ]] \
    || [[ -z "$(find "$DIFF_DIR" -mindepth 1 -maxdepth 1 -type d)" ]]; then
    echo "ERROR: No GNU test results in $DIFF_DIR" >&2
    echo "  The GNU test run failed or did not run any tests." >&2
    exit 2
fi

# Collect current failures. The GNU harness (ts_gen_diff in
# tests/functions.sh) fails a test if its stdout or its stderr differs from
# the expected output, writing <test> or <test>.err respectively, so a test
# whose stdout matches but whose stderr differs leaves only <test>.err.
# Map both to <test> and de-duplicate so such failures are counted too.
current_tmp=$(mktemp)
trap 'rm -f "$current_tmp"' EXIT
find "$DIFF_DIR" -mindepth 2 -maxdepth 2 -type f -print0 \
    | while IFS= read -r -d '' path; do
        path="${path#"$DIFF_DIR/"}"
        echo "${path%.err}"
      done \
    | sort -u > "$current_tmp"

# --update-baseline mode
if [[ "$UPDATE_BASELINE" == true ]]; then
    {
        echo "# Known GNU test failures - format: util/testname"
        echo "# Must match CI: run against the util-linux version checked out in"
        echo "# .github/workflows/run-gnu-test.yml."
        echo "# Update with: ./scripts/check-new-gnu-failures.sh --update-baseline"
        cat "$current_tmp"
    } > "$BASELINE"
    count=$(wc -l < "$current_tmp")
    echo "Baseline updated: $BASELINE ($count failures recorded)"
    exit 0
fi

# The baseline is checked in, so a missing file is an error rather than a
# reason to create one: creating it from the current results would accept
# every current failure, including regressions, and pass.
if [[ ! -f "$BASELINE" ]]; then
    echo "ERROR: Baseline not found: $BASELINE" >&2
    echo "  Restore it from version control, or create it with:" >&2
    echo "    ./scripts/check-new-gnu-failures.sh --update-baseline" >&2
    exit 2
fi

# Load baseline (strip comments and blank lines)
baseline_tmp=$(mktemp)
trap 'rm -f "$current_tmp" "$baseline_tmp"' EXIT
# Use sed rather than grep -v: grep exits 1 when it selects no lines,
# which pipefail and set -e would turn into an abort for a baseline with
# no entries (e.g. after every known failure has been fixed).
sed -e '/^[[:space:]]*#/d' -e '/^[[:space:]]*$/d' "$BASELINE" \
    | sort > "$baseline_tmp"

# As in the check on DIFF_DIR above: if a component with known failures
# has no DIFF_DIR/<component>, none of its tests produced results (e.g.
# they were all skipped, or the utility is no longer tested), so its known
# failures must not be reported as fixed.
missing_components=$(cut -d/ -f1 "$baseline_tmp" | sort -u \
    | while IFS= read -r component; do
        [[ -d "$DIFF_DIR/$component" ]] || echo "$component"
      done)
if [[ -n "$missing_components" ]]; then
    echo "ERROR: No GNU test results for components with known failures:" >&2
    awk '{print "  " $0}' <<< "$missing_components" >&2
    echo "  Check the GNU test output for skipped tests, or remove the" >&2
    echo "  components' entries from the baseline if they are no longer tested." >&2
    exit 2
fi

# comm: -13 = lines only in current (new failures), -23 = lines only in baseline (fixed)
new_failures=$(comm -13 "$baseline_tmp" "$current_tmp")
fixed_tests=$(comm -23 "$baseline_tmp" "$current_tmp")

# Report
echo "--- GNU test failure summary ---"
echo "Baseline failures: $(wc -l < "$baseline_tmp")"
echo "Current failures:  $(wc -l < "$current_tmp")"
echo ""

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
