#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

ISO_PATH="${REPO_ROOT}/target/postcard-spatial-os.iso"
DISK_PATH="${REPO_ROOT}/target/postcard-os.raw"

BOOT_MODE="auto" # auto, iso, disk
MEMORY="256M"
DISPLAY_OPT=""
EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --iso)
            BOOT_MODE="iso"
            shift
            ;;
        --disk)
            BOOT_MODE="disk"
            if [[ $# -gt 1 && ! "$2" =~ ^-- ]]; then
                DISK_PATH="$2"
                shift
            fi
            shift
            ;;
        --headless)
            DISPLAY_OPT="-display none"
            shift
            ;;
        -m|--memory)
            MEMORY="$2"
            shift 2
            ;;
        *)
            EXTRA_ARGS+=("$1")
            shift
            ;;
    esac
done

if [[ "$BOOT_MODE" == "auto" ]]; then
    if [[ -f "$ISO_PATH" ]]; then
        BOOT_MODE="iso"
    elif [[ -f "$DISK_PATH" ]]; then
        BOOT_MODE="disk"
    else
        echo "No boot media found. Building ISO..."
        "${SCRIPT_DIR}/create_iso.sh"
        BOOT_MODE="iso"
    fi
fi

QEMU_CMD=(qemu-system-x86_64 -name "Post Card OS" -machine pc -m "$MEMORY" -serial mon:stdio -no-reboot)

if [[ -n "$DISPLAY_OPT" ]]; then
    QEMU_CMD+=($DISPLAY_OPT)
fi

if [[ "$BOOT_MODE" == "iso" ]]; then
    if [[ ! -f "$ISO_PATH" ]]; then
        echo "Error: ISO not found at $ISO_PATH. Run scripts/create_bootable_iso.sh first." >&2
        exit 1
    fi
    echo "Booting Post Card OS from ISO ($ISO_PATH)..."
    QEMU_CMD+=(-cdrom "$ISO_PATH" -boot d)
else
    if [[ ! -f "$DISK_PATH" ]]; then
        echo "Error: Disk image not found at $DISK_PATH." >&2
        exit 1
    fi
    echo "Booting Post Card OS from raw disk ($DISK_PATH)..."
    QEMU_CMD+=(-drive "file=$DISK_PATH,format=raw,if=ide,index=0" -boot order=c)
fi

if [[ ${#EXTRA_ARGS[@]} -gt 0 ]]; then
    QEMU_CMD+=("${EXTRA_ARGS[@]}")
fi

exec "${QEMU_CMD[@]}"
