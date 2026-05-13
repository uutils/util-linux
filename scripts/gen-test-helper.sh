#!/bin/bash

set -euxo pipefail

cd "$(dirname "$0")/../"
PROJECT_DIR="$(pwd)"
BINARY="$PROJECT_DIR/target/release/util-linux"

if [[ -z "${GNU_PROJECT_DIR:-}" ]]; then
    echo "ERROR: GNU_PROJECT_DIR is not set." >&2
    echo "  Set it to the path of a gnu-util-linux checkout." >&2
    exit 2
fi

GNU_TS_DIR="$GNU_PROJECT_DIR/tests/ts"
if [[ ! -d "$GNU_TS_DIR" ]]; then
    echo "ERROR: GNU test directory not found: $GNU_TS_DIR" >&2
    exit 2
fi

mkdir -p .test-helpers

ours_tmp=$(mktemp)
gnu_tmp=$(mktemp)
trap 'rm -f "$ours_tmp" "$gnu_tmp"' EXIT

for d in "$PROJECT_DIR/src/uu/"*/; do
    basename "$d"
done | sort > "$ours_tmp"

for d in "$GNU_TS_DIR/"*/; do
    basename "$d"
done | sort > "$gnu_tmp"

comm -12 "$ours_tmp" "$gnu_tmp" > "$PROJECT_DIR/.test-helpers/utils.list"

mapfile -t UTILS < "$PROJECT_DIR/.test-helpers/utils.list"

if [[ ${#UTILS[@]} -eq 0 ]]; then
    echo "ERROR: No utilities matched between src/uu/ and $GNU_TS_DIR/" >&2
    exit 1
fi

for util in "${UTILS[@]}"; do
    cat > ".test-helpers/$util" <<EOF
#!/bin/bash
exec "$BINARY" $util "\$@"
EOF
    chmod +x ".test-helpers/$util"
done

echo "Generated wrappers for ${#UTILS[@]} utilities: ${UTILS[*]}"
