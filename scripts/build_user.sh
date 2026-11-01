#!/usr/bin/env bash
set -euo pipefail

# Determine repository root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Handle --count / --verify CLI inspection mode
if [[ "${1:-}" == "--count" || "${1:-}" == "--verify" ]]; then
    IMG="${2:-${REPO_ROOT}/ramdisk.img}"
    python3 - "${IMG}" << 'EOF'
import struct, sys
img_path = sys.argv[1]
try:
    with open(img_path, "rb") as f:
        data = f.read()
    if len(data) < 8 or data[:4] != b"RDSK":
        print(0)
        sys.exit(0)
    count = struct.unpack("<I", data[4:8])[0]
    print(count)
except Exception:
    print(0)
EOF
    exit 0
fi

# Ensure cargo is in PATH
if ! command -v cargo &>/dev/null; then
    if [[ -f "${HOME}/.cargo/env" ]]; then
        # shellcheck source=/dev/null
        source "${HOME}/.cargo/env"
    fi
fi

if ! command -v cargo &>/dev/null; then
    echo "Error: cargo not found in PATH or ~/.cargo/env" >&2
    exit 1
fi

export CARGO_INCREMENTAL=0

USER_BINARIES="${USER_BINARIES:-hello evil poke peek}"
for bin_name in ${USER_BINARIES}; do
    if [[ ${#bin_name} -gt 24 ]]; then
        echo "Error: Binary name '${bin_name}' exceeds 24 bytes" >&2
        exit 1
    fi
done

echo "=== Building userland binaries ==="
USER_DIR="${REPO_ROOT}/user"
USER_LD="${USER_DIR}/user.ld"
BIN_DIR="${USER_DIR}/target/x86_64-unknown-none/release"
RAMDISK_IMG="${REPO_ROOT}/ramdisk.img"

if [[ ! -f "${USER_LD}" ]]; then
    echo "Error: user linker script not found at ${USER_LD}" >&2
    exit 1
fi

(
    cd "${USER_DIR}"
    RUSTFLAGS="-C link-arg=-T${USER_LD} -C relocation-model=static -C code-model=large" \
        cargo build --release --target x86_64-unknown-none
)

echo "=== Packing ramdisk.img ==="
python3 - "${BIN_DIR}" "${RAMDISK_IMG}" ${USER_BINARIES} << 'EOF'
import os
import struct
import sys

bin_dir = sys.argv[1]
out_img = sys.argv[2]
names = sys.argv[3:] if len(sys.argv) > 3 else ["hello", "evil", "poke", "peek"]

payloads = []
for name in names:
    assert len(name) <= 24, f"Binary name '{name}' exceeds 24 bytes"
    path = os.path.join(bin_dir, name)
    if not os.path.isfile(path):
        raise FileNotFoundError(f"Missing user binary: {path}")
    with open(path, "rb") as f:
        data = f.read()
    payloads.append((name, data))

count = len(payloads)
header_size = 8 + 32 * count
entries = []
offset = header_size

for name, data in payloads:
    name_bytes = name.encode("ascii").ljust(24, b"\x00")
    size = len(data)
    entries.append((name_bytes, offset, size))
    offset += size

with open(out_img, "wb") as f:
    f.write(b"RDSK")
    f.write(struct.pack("<I", count))
    for name_bytes, off, sz in entries:
        f.write(name_bytes)
        f.write(struct.pack("<II", off, sz))
    for _, data in payloads:
        f.write(data)

print(f"ramdisk.img successfully created: {offset} bytes, {count} programs")
EOF

chmod 0644 "${RAMDISK_IMG}"
echo "Userland build and ramdisk packaging complete."
