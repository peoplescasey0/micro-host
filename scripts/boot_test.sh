#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

TIMEOUT_SECS=12
ISO_PATH="${REPO_ROOT}/target/postcard-spatial-os.iso"
DISK_PATH="${REPO_ROOT}/target/postcard-os.raw"
SERIAL_LOG="${REPO_ROOT}/target/serial.log"
USE_DISK=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --log|--serial-log)
            SERIAL_LOG="$2"
            shift 2
            ;;
        --timeout|-t)
            TIMEOUT_SECS="$2"
            shift 2
            ;;
        --disk)
            USE_DISK="1"
            if [[ $# -gt 1 && ! "$2" =~ ^-- ]]; then
                DISK_PATH="$2"
                shift
            fi
            shift
            ;;
        *)
            if [[ "$1" =~ ^[0-9]+$ ]]; then
                TIMEOUT_SECS="$1"
            fi
            shift
            ;;
    esac
done

mkdir -p "$(dirname "$SERIAL_LOG")"
rm -f "$SERIAL_LOG"

if [[ -n "$USE_DISK" ]]; then
    if [[ ! -f "$DISK_PATH" ]]; then
        echo "Raw disk not found at $DISK_PATH, building via install_postcard_os_wsl.sh..."
        "${SCRIPT_DIR}/install_postcard_os_wsl.sh" "$DISK_PATH" "${REPO_ROOT}/target/x86_64-unknown-none/release/postcard-minimal-kernel"
    fi
    echo "Running boot test on raw disk: $DISK_PATH (timeout: ${TIMEOUT_SECS}s)..."
    BOOT_ARGS=(-drive "file=$DISK_PATH,format=raw,if=ide,index=0,snapshot=on" -boot order=c)
else
    if [[ ! -f "$ISO_PATH" ]]; then
        echo "ISO not found, building..."
        "${SCRIPT_DIR}/create_iso.sh"
    fi
    echo "Running boot test on ISO: $ISO_PATH (timeout: ${TIMEOUT_SECS}s)..."
    BOOT_ARGS=(-cdrom "$ISO_PATH" -boot d)
fi

qemu-system-x86_64 \
    -name "Post Card OS Boot Test" \
    -machine pc \
    -m 256M \
    "${BOOT_ARGS[@]}" \
    -display none \
    -serial "file:${SERIAL_LOG}" \
    -no-reboot &
QEMU_PID=$!

cleanup() {
    if kill -0 "$QEMU_PID" 2>/dev/null; then
        kill "$QEMU_PID" 2>/dev/null || true
        wait "$QEMU_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

START_TIME=$(date +%s)
BOOT_SUCCESS=0

while true; do
    CURRENT_TIME=$(date +%s)
    ELAPSED=$((CURRENT_TIME - START_TIME))
    
    if [[ -f "$SERIAL_LOG" ]] && grep -q "spatial>" "$SERIAL_LOG" 2>/dev/null; then
        BOOT_SUCCESS=1
        break
    fi
    
    if ! kill -0 "$QEMU_PID" 2>/dev/null; then
        echo "QEMU exited unexpectedly before reaching shell prompt."
        break
    fi
    
    if [[ $ELAPSED -ge $TIMEOUT_SECS ]]; then
        echo "Timeout (${TIMEOUT_SECS}s) reached waiting for boot prompt."
        break
    fi
    
    sleep 0.2
done

cleanup
trap - EXIT INT TERM

echo "=== Serial Log Output ==="
if [[ -f "$SERIAL_LOG" ]]; then
    cat "$SERIAL_LOG"
else
    echo "No serial log produced!" >&2
    exit 1
fi
echo "========================="

if [[ $BOOT_SUCCESS -ne 1 ]]; then
    echo "Boot test FAILED: Prompt 'spatial>' was not reached." >&2
    exit 1
fi

REQUIRED_STRINGS=(
    "[ok] GDT + TSS loaded"
    "[ok] IDT loaded"
    "[ok] heap:"
    "[ok] kernel page tables active"
    "[ok] timer IRQ:"
    "[ok] scheduler:"
    "[ok] user mode: ring 3 process ran"
    "[ok] address spaces: same virtual address, separate memory"
    "Spatial OS is ALIVE!"
    "spatial>"
)

EXTRA_STRINGS=(
    "[ok] PCI + BitVisor/OpenShell VFIO:"
    "[ok] storage + MBR + I/O spooler:"
    "[ok] OpenShell/OpenSandbox contract:"
)

FAILED_ASSERTIONS=0
for req in "${REQUIRED_STRINGS[@]}"; do
    if ! grep -Fq "$req" "$SERIAL_LOG"; then
        echo "Assertion FAILED: Missing required output: '$req'" >&2
        FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
    fi
done

for ext in "${EXTRA_STRINGS[@]}"; do
    if grep -Fq "$ext" "$SERIAL_LOG"; then
        echo "Subsystem verified: '$ext'"
    fi
done

if [[ $FAILED_ASSERTIONS -gt 0 ]]; then
    echo "Boot test FAILED with $FAILED_ASSERTIONS failed assertions." >&2
    exit 1
fi

echo "Boot test PASSED: All kernel self-tests and boot prompt verified successfully."
exit 0
