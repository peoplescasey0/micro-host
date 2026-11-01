#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

TIMEOUT_SECS=30
ISO_PATH="${REPO_ROOT}/target/postcard-spatial-os.iso"
DISK_PATH="${REPO_ROOT}/target/postcard-os.raw"
LOG_PATH="${REPO_ROOT}/target/shell.log"
USE_DISK=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --log|--serial-log)
            LOG_PATH="$2"
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
        --iso)
            ISO_PATH="$2"
            shift 2
            ;;
        *)
            if [[ "$1" =~ ^[0-9]+$ ]]; then
                TIMEOUT_SECS="$1"
            fi
            shift
            ;;
    esac
done

mkdir -p "$(dirname "$LOG_PATH")"

if [[ -n "$USE_DISK" ]]; then
    if [[ ! -f "$DISK_PATH" ]]; then
        echo "Raw disk not found at $DISK_PATH, building via install_postcard_os_wsl.sh..."
        "${SCRIPT_DIR}/install_postcard_os_wsl.sh" "$DISK_PATH" "${REPO_ROOT}/target/x86_64-unknown-none/release/postcard-minimal-kernel"
    fi
    MEDIA_ARG="--disk $DISK_PATH"
else
    if [[ ! -f "$ISO_PATH" ]]; then
        echo "ISO not found, building..."
        "${SCRIPT_DIR}/create_iso.sh"
    fi
    MEDIA_ARG="--iso $ISO_PATH"
fi

echo "=== Running QEMU Interactive Serial Shell Test ==="

python3 - "${MEDIA_ARG}" "${TIMEOUT_SECS}" "${LOG_PATH}" << 'EOF'
import os
import select
import signal
import subprocess
import sys
import time

media_type = sys.argv[1].split()[0] # --iso or --disk
media_file = sys.argv[1].split()[1]
timeout_secs = float(sys.argv[2])
log_path = sys.argv[3] if len(sys.argv) > 3 else ""

if log_path:
    os.makedirs(os.path.dirname(os.path.abspath(log_path)), exist_ok=True)
    log_file = open(log_path, "w", encoding="utf-8")
else:
    log_file = None

def log_write(text):
    if log_file:
        log_file.write(text)
        log_file.flush()

qemu_cmd = [
    "qemu-system-x86_64",
    "-name", "Post Card OS Shell Test",
    "-machine", "pc",
    "-m", "256M",
    "-display", "none",
    "-serial", "stdio",
    "-no-reboot"
]

if media_type == "--iso":
    qemu_cmd.extend(["-cdrom", media_file, "-boot", "d"])
else:
    qemu_cmd.extend(["-drive", f"file={media_file},format=raw,if=ide,index=0,snapshot=on", "-boot", "order=c"])

print(f"Launching QEMU ({media_type} {media_file})...")
proc = subprocess.Popen(
    qemu_cmd,
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.STDOUT,
    text=False,
    bufsize=0
)

def read_until(pattern, timeout):
    buf = ""
    start = time.time()
    while time.time() - start < timeout:
        if proc.poll() is not None:
            try:
                drain = os.read(proc.stdout.fileno(), 4096).decode("utf-8", errors="replace")
                if drain:
                    buf += drain
                    log_write(drain)
            except Exception:
                pass
            if pattern in buf:
                return buf
            raise RuntimeError(
                f"QEMU terminated prematurely with exit code {proc.returncode} "
                f"while waiting for '{pattern}'. Received so far:\n{buf}"
            )
        remaining = max(0.01, timeout - (time.time() - start))
        rlist, _, _ = select.select([proc.stdout], [], [], min(remaining, 0.2))
        if rlist:
            try:
                raw = os.read(proc.stdout.fileno(), 1024)
            except Exception:
                raw = b""
            if not raw:
                if pattern in buf:
                    return buf
                raise RuntimeError(
                    f"EOF on QEMU stdout while waiting for '{pattern}' (exit code {proc.poll()}). "
                    f"Received so far:\n{buf}"
                )
            chunk = raw.decode("utf-8", errors="replace")
            buf += chunk
            log_write(chunk)
            if pattern in buf:
                return buf
    raise TimeoutError(f"Timed out ({timeout}s) waiting for '{pattern}'. Received so far:\n{buf}")

def cleanup():
    if proc.poll() is None:
        try:
            proc.stdin.write(b"halt\n")
            proc.stdin.flush()
            time.sleep(0.3)
        except Exception:
            pass
    try:
        if proc.stdin and not proc.stdin.closed:
            proc.stdin.close()
    except Exception:
        pass
    if proc.poll() is None:
        proc.terminate()
        try:
            proc.wait(timeout=2.0)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
    try:
        if proc.stdout and not proc.stdout.closed:
            proc.stdout.close()
    except Exception:
        pass
    try:
        if log_file and not log_file.closed:
            log_file.close()
    except Exception:
        pass

signal.signal(signal.SIGINT, lambda s, f: (cleanup(), sys.exit(130)))
signal.signal(signal.SIGTERM, lambda s, f: (cleanup(), sys.exit(143)))

try:
    print("Waiting for boot to reach 'spatial>' prompt...")
    boot_log = read_until("spatial> ", timeout=timeout_secs)
    print("Initial shell prompt reached.")

    test_commands = [
        ("help", ["echo", "uptime", "ps", "disks"]),
        ("uptime", ["ticks)"]),
        ("mem", ["frames"]),
        ("ps", ["shell", "running"]),
        ("pci", ["class="]),
        ("disks", ["ramdisk:"]),
        ("sandbox", ["sandbox identity:"]),
        ("foobar_unknown_cmd", ["unknown command: foobar_unknown_cmd"])
    ]

    for cmd, expected_substrings in test_commands:
        print(f"Executing shell command: '{cmd}'")
        log_write(f"spatial> {cmd}\n")
        proc.stdin.write(f"{cmd}\n".encode("utf-8"))
        proc.stdin.flush()
        
        output = read_until("spatial> ", timeout=10.0)
        for expected in expected_substrings:
            if expected not in output:
                raise AssertionError(f"Command '{cmd}' output missing expected substring '{expected}'. Output:\n{output}")
        print(f"  [OK] '{cmd}' returned expected output.")

    print("\nAll shell tests passed successfully!")

finally:
    cleanup()

EOF

echo "QEMU serial shell test completed successfully."
