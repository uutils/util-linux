#!/bin/bash

set -euxo pipefail

UTILS=(cal dmesg hexdump lscpu lslocks lsmem)

cd "$(dirname "$0")/../"
PROJECT_DIR="$(pwd)"
BINARY="$PROJECT_DIR/target/release/util-linux"

mkdir -p .test-helpers


for util in "${UTILS[@]}"; do
    cat > ".test-helpers/$util" <<EOF
#!/bin/bash
exec "$BINARY" $util "\$@"
EOF
    chmod +x ".test-helpers/$util"
done

