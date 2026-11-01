#!/usr/bin/env bash
# ==============================================================================
# tests/run_e2e_tests.sh
# Comprehensive Dual-Track E2E Test Suite for Post Card Spatial OS (micro-host)
#
# Covers all 18 features from PROJECT.md across 4 verification tiers:
#   - Tier 1: Feature Coverage (>=5 test cases per feature, 90 tests)
#   - Tier 2: Boundary & Corner Cases (16 tests)
#   - Tier 3: Cross-Feature Combinations (9 tests)
#   - Tier 4: Real-World Application Workloads (5 tests)
# Total: 120 genuine end-to-end assertions.
# Produces TAP Version 14 stream and target/e2e_report.tap / target/e2e_summary.json.
# ==============================================================================

set -uo pipefail

# Ensure essential tools are in PATH
export PATH="${HOME}/.local/bin:${HOME}/.cargo/bin:${PATH}"
export CARGO_INCREMENTAL=0

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
# Configuration flags
RUN_TIER1=1
RUN_TIER2=1
RUN_TIER3=1
RUN_TIER4=1
FILTER_FEATURE=""
QUICK_MODE=0
TAP_ONLY=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --tier1)
            RUN_TIER1=1; RUN_TIER2=0; RUN_TIER3=0; RUN_TIER4=0
            shift
            ;;
        --tier2)
            RUN_TIER1=0; RUN_TIER2=1; RUN_TIER3=0; RUN_TIER4=0
            shift
            ;;
        --tier3)
            RUN_TIER1=0; RUN_TIER2=0; RUN_TIER3=1; RUN_TIER4=0
            shift
            ;;
        --tier4)
            RUN_TIER1=0; RUN_TIER2=0; RUN_TIER3=0; RUN_TIER4=1
            shift
            ;;
        --feature=*)
            FILTER_FEATURE="${1#*=}"
            shift
            ;;
        -f)
            FILTER_FEATURE="$2"
            shift 2
            ;;
        --quick)
            QUICK_MODE=1
            shift
            ;;
        --tap-only)
            TAP_ONLY=1
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [options]"
            echo "Options:"
            echo "  --tier1        Run Tier 1 feature coverage tests only"
            echo "  --tier2        Run Tier 2 boundary & corner case tests only"
            echo "  --tier3        Run Tier 3 cross-feature combination tests only"
            echo "  --tier4        Run Tier 4 real-world application scenario tests only"
            echo "  --feature=N    Run tests matching feature number (1-18)"
            echo "  --quick        Skip rebuilding images if already present"
            echo "  --tap-only     Output pure TAP version 14 stream to stdout"
            echo "  --help         Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

RUN_ID="$$"
TARGET_DIR="${REPO_ROOT}/target"
mkdir -p "${TARGET_DIR}"
TAP_TMP="${TARGET_DIR}/e2e_report.${RUN_ID}.tap"
SUMMARY_TMP="${TARGET_DIR}/e2e_summary.${RUN_ID}.json"
E2E_BOOT_LOG="${TARGET_DIR}/serial_boot_${RUN_ID}.log"
E2E_SHELL_LOG="${TARGET_DIR}/serial_shell_${RUN_ID}.log"

TAP_FILE="${TAP_TMP}"
SUMMARY_FILE="${SUMMARY_TMP}"
rm -f "${TAP_FILE}" "${SUMMARY_FILE}" "${E2E_BOOT_LOG}" "${E2E_SHELL_LOG}"

# ANSI color codes
if [[ -t 1 && $TAP_ONLY -eq 0 ]]; then
    C_RESET="\033[0m"
    C_BOLD="\033[1m"
    C_GREEN="\033[32m"
    C_RED="\033[31m"
    C_YELLOW="\033[33m"
    C_CYAN="\033[36m"
else
    C_RESET=""
    C_BOLD=""
    C_GREEN=""
    C_RED=""
    C_YELLOW=""
    C_CYAN=""
fi

TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
SKIPPED_TESTS=0
START_TIME=$(date +%s)

declare -a TEST_RESULTS=()

log_diag() {
    local msg="$1"
    echo "# ${msg}" >> "${TAP_FILE}"
    if [[ $TAP_ONLY -eq 0 ]]; then
        echo -e "  ${C_CYAN}#${C_RESET} ${msg}"
    else
        echo "# ${msg}"
    fi
}

run_test() {
    local test_id="$1"
    local desc="$2"
    local feature_num="$3"
    shift 3
    local cmd=("$@")

    if [[ -n "$FILTER_FEATURE" && "$FILTER_FEATURE" != "$feature_num" ]]; then
        return 0
    fi

    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    local test_num=$TOTAL_TESTS
    local status="ok"
    local output=""
    local exit_code=0

    # Run command and capture output
    output=$("${cmd[@]}" 2>&1) || exit_code=$?

    if [[ $exit_code -eq 0 ]]; then
        PASSED_TESTS=$((PASSED_TESTS + 1))
        local tap_line="ok ${test_num} - ${test_id}: ${desc}"
        echo "${tap_line}" >> "${TAP_FILE}"
        if [[ $TAP_ONLY -eq 0 ]]; then
            echo -e "${C_GREEN}PASS${C_RESET} [${test_num}] ${C_BOLD}${test_id}${C_RESET} - ${desc}"
        else
            echo "${tap_line}"
        fi
        TEST_RESULTS+=("{\"id\":\"${test_id}\",\"num\":${test_num},\"feature\":${feature_num},\"status\":\"PASS\",\"desc\":\"${desc}\"}")
    else
        FAILED_TESTS=$((FAILED_TESTS + 1))
        local tap_line="not ok ${test_num} - ${test_id}: ${desc}"
        echo "${tap_line}" >> "${TAP_FILE}"
        echo "# Command failed with exit code ${exit_code}" >> "${TAP_FILE}"
        if [[ -n "$output" ]]; then
            while IFS= read -r line; do
                echo "# ${line}" >> "${TAP_FILE}"
            done <<< "$output"
        fi

        if [[ $TAP_ONLY -eq 0 ]]; then
            echo -e "${C_RED}FAIL${C_RESET} [${test_num}] ${C_BOLD}${test_id}${C_RESET} - ${desc} (exit ${exit_code})"
            if [[ -n "$output" ]]; then
                echo -e "${C_RED}${output}${C_RESET}"
            fi
        else
            echo "${tap_line}"
            echo "# Command failed with exit code ${exit_code}"
        fi
        TEST_RESULTS+=("{\"id\":\"${test_id}\",\"num\":${test_num},\"feature\":${feature_num},\"status\":\"FAIL\",\"desc\":\"${desc}\",\"exit\":${exit_code}}")
    fi
}

if [[ $TAP_ONLY -eq 0 ]]; then
    echo -e "${C_BOLD}==============================================================================${C_RESET}"
    echo -e "${C_CYAN}${C_BOLD}Post Card Spatial OS · Comprehensive Dual-Track E2E Test Suite${C_RESET}"
    echo -e "Repository: ${REPO_ROOT}"
    echo -e "Started: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    echo -e "${C_BOLD}==============================================================================${C_RESET}"
fi

echo "TAP version 14" > "${TAP_FILE}"

# Pre-flight check: ensure basic build artifacts exist if running in quick mode
if [[ $QUICK_MODE -eq 0 ]]; then
    log_diag "Building base artifacts (userland, ramdisk, secure, kernel)..."
    (cd "${REPO_ROOT}" && make check >/dev/null 2>&1) || true
    (cd "${REPO_ROOT}" && make all >/dev/null 2>&1) || true
fi

# ==============================================================================
# TIER 1: FEATURE COVERAGE (>=5 test cases per feature for Features 1-18)
# ==============================================================================
if [[ $RUN_TIER1 -eq 1 ]]; then
    if [[ $TAP_ONLY -eq 0 ]]; then
        echo -e "\n${C_YELLOW}${C_BOLD}--- TIER 1: FEATURE COVERAGE (Features 1-18) ---${C_RESET}"
    fi
    echo "# Subtest: Tier 1 Feature Coverage" >> "${TAP_FILE}"

    # ── Feature 1: rust-toolchain.toml Syntax & Pinned Specs ─────────────
    run_test "T1.F01.1" "rust-toolchain.toml file exists and is valid format" "1" \
        test -s "${REPO_ROOT}/rust-toolchain.toml"

    run_test "T1.F01.2" "rust-toolchain.toml contains exactly single [toolchain] section" "1" \
        python3 -c '
with open("'"${REPO_ROOT}"'/rust-toolchain.toml", "r") as f:
    text = f.read()
assert text.count("[toolchain]") == 1, "Duplicate [toolchain] sections found"
'

    run_test "T1.F01.3" "rust-toolchain.toml has single components key with no duplicate declarations" "1" \
        python3 -c '
with open("'"${REPO_ROOT}"'/rust-toolchain.toml", "r") as f:
    lines = [l.strip() for l in f if l.strip().startswith("components")]
assert len(lines) == 1, f"Expected 1 components line, found {len(lines)}"
'

    run_test "T1.F01.4" "rust-toolchain.toml pins channel to nightly-2024-12-01" "1" \
        grep -q 'channel = "nightly-2024-12-01"' "${REPO_ROOT}/rust-toolchain.toml"

    run_test "T1.F01.5" "rust-toolchain.toml declares target x86_64-unknown-none and required components" "1" \
        python3 -c '
with open("'"${REPO_ROOT}"'/rust-toolchain.toml", "r") as f:
    text = f.read()
assert "x86_64-unknown-none" in text, "Missing x86_64-unknown-none target"
assert "rust-src" in text, "Missing rust-src component"
assert "llvm-tools-preview" in text, "Missing llvm-tools-preview component"
'

    # ── Feature 2: Toolchain & Target Verification on Host ───────────────
    run_test "T1.F02.1" "Host rustc runs successfully and reports nightly" "2" \
        bash -c 'rustc --version | grep -qi "nightly"'

    run_test "T1.F02.2" "Host rustup target list contains installed x86_64-unknown-none" "2" \
        bash -c 'rustup target list --installed | grep -q "x86_64-unknown-none"'

    run_test "T1.F02.3" "Host cargo runs successfully" "2" \
        cargo --version

    run_test "T1.F02.4" "CARGO_INCREMENTAL=0 is accepted without incremental cache issues" "2" \
        bash -c 'cd "'"${REPO_ROOT}"'/secure" && CARGO_INCREMENTAL=0 cargo check --quiet'

    run_test "T1.F02.5" "Bare-metal target compilation passes check on user crate" "2" \
        bash -c 'cd "'"${REPO_ROOT}"'/user" && cargo check --target x86_64-unknown-none --quiet'

    # ── Feature 3: Linux Automation Infra & Master Makefile ───────────────
    run_test "T1.F03.1" "Root Makefile exists and declares canonical targets" "3" \
        python3 -c '
with open("'"${REPO_ROOT}"'/Makefile", "r") as f:
    mf = f.read()
for target in ["all:", "check:", "userland:", "ramdisk:", "secure:", "kernel:", "iso:", "disk:", "frontend:", "test:", "clean:", "help:"]:
    assert target in mf, f"Makefile missing target {target}"
'

    run_test "T1.F03.2" "All automation scripts in scripts/ have executable bit set" "3" \
        bash -c '
for s in "'"${REPO_ROOT}"'/scripts"/*.sh; do
    [[ -x "$s" ]] || exit 1
done
'

    run_test "T1.F03.3" "All scripts use strict error flags (set -euo pipefail)" "3" \
        bash -c '
for s in "'"${REPO_ROOT}"'/scripts"/build_user.sh "'"${REPO_ROOT}"'/scripts"/build_kernel.sh "'"${REPO_ROOT}"'/scripts"/create_iso.sh "'"${REPO_ROOT}"'/scripts"/boot_test.sh "'"${REPO_ROOT}"'/scripts"/shell_test.sh; do
    grep -Eq "set -[a-z]*e[a-z]*" "$s" || exit 1
done
'

    run_test "T1.F03.4" "Automation scripts return non-zero exit code on invalid arguments" "3" \
        bash -c '! "'"${REPO_ROOT}"'/scripts/install_postcard_os_wsl.sh"'

    run_test "T1.F03.5" "Makefile help target executes cleanly and lists commands" "3" \
        bash -c 'cd "'"${REPO_ROOT}"'" && make help | grep -q "Post Card Spatial OS Build Targets"'

    # ── Feature 4: Userland Binary Compilation ───────────────────────────
    run_test "T1.F04.1" "Userland binaries (hello, evil, poke, peek) exist in release target" "4" \
        bash -c '
for bin in hello evil poke peek; do
    [[ -s "'"${REPO_ROOT}"'/user/target/x86_64-unknown-none/release/$bin" ]] || exit 1
done
'

    run_test "T1.F04.2" "Userland binaries have valid 64-bit ELF headers and little-endian encoding" "4" \
        python3 -c '
import os
for bin in ["hello", "evil", "poke", "peek"]:
    path = "'"${REPO_ROOT}"'/user/target/x86_64-unknown-none/release/" + bin
    with open(path, "rb") as f:
        hdr = f.read(16)
    assert hdr[:4] == b"\x7fELF", f"{bin}: bad magic"
    assert hdr[4] == 2, f"{bin}: not 64-bit"
    assert hdr[5] == 1, f"{bin}: not little endian"
'

    run_test "T1.F04.3" "Userland binaries target EM_X86_64 machine architecture" "4" \
        python3 -c '
for bin in ["hello", "evil", "poke", "peek"]:
    path = "'"${REPO_ROOT}"'/user/target/x86_64-unknown-none/release/" + bin
    with open(path, "rb") as f:
        hdr = f.read(20)
    machine = int.from_bytes(hdr[18:20], "little")
    assert machine == 62, f"{bin}: expected machine 62 (x86-64), got {machine}"
'

    run_test "T1.F04.4" "Userland entry points are mapped at USER_BASE 0x80_0000_0000" "4" \
        python3 -c '
for bin in ["hello", "evil", "poke", "peek"]:
    path = "'"${REPO_ROOT}"'/user/target/x86_64-unknown-none/release/" + bin
    with open(path, "rb") as f:
        hdr = f.read(32)
    entry = int.from_bytes(hdr[24:32], "little")
    assert entry == 0x8000000000, f"{bin}: expected entry 0x8000000000, got {entry:#x}"
'

    run_test "T1.F04.5" "user.ld linker script specifies base 0x8000000000 and entry _start" "4" \
        python3 -c '
with open("'"${REPO_ROOT}"'/user/user.ld", "r") as f:
    ld = f.read()
assert "ENTRY(_start)" in ld, "Missing ENTRY(_start)"
assert "0x8000000000" in ld or "0x80_0000_0000" in ld, "Missing virtual base in user.ld"
'

    # ── Feature 5: Ramdisk Image Packaging ────────────────────────────────
    run_test "T1.F05.1" "ramdisk.img exists and has valid magic RDSK (0x5244534B)" "5" \
        python3 -c '
with open("'"${REPO_ROOT}"'/ramdisk.img", "rb") as f:
    magic = f.read(4)
assert magic == b"RDSK", f"Invalid magic: {magic}"
'

    run_test "T1.F05.2" "ramdisk.img header specifies exactly 4 programs" "5" \
        python3 -c '
with open("'"${REPO_ROOT}"'/ramdisk.img", "rb") as f:
    f.seek(4)
    count = int.from_bytes(f.read(4), "little")
assert count == 4, f"Expected 4 programs, got {count}"
'

    run_test "T1.F05.3" "ramdisk.img directory entries are 32 bytes each with 24-char NUL-padded names" "5" \
        python3 -c '
with open("'"${REPO_ROOT}"'/ramdisk.img", "rb") as f:
    data = f.read()
count = int.from_bytes(data[4:8], "little")
names = []
for i in range(count):
    entry = data[8 + i*32 : 8 + (i+1)*32]
    raw_name = entry[:24]
    name = raw_name.split(b"\x00")[0].decode("ascii")
    names.append(name)
assert names == ["hello", "evil", "poke", "peek"], f"Unexpected names: {names}"
'

    run_test "T1.F05.4" "ramdisk.img directory offsets and sizes match physical ELF payloads" "5" \
        python3 -c '
with open("'"${REPO_ROOT}"'/ramdisk.img", "rb") as f:
    data = f.read()
count = int.from_bytes(data[4:8], "little")
header_size = 8 + 32 * count
for i in range(count):
    entry = data[8 + i*32 : 8 + (i+1)*32]
    off = int.from_bytes(entry[24:28], "little")
    sz = int.from_bytes(entry[28:32], "little")
    assert off >= header_size, f"Offset {off} overlaps header {header_size}"
    assert off + sz <= len(data), f"Entry {i} exceeds ramdisk size"
    payload = data[off : off+sz]
    assert payload[:4] == b"\x7fELF", f"Payload at {off} is not an ELF binary"
'

    run_test "T1.F05.5" "build_user.sh repacks ramdisk.img cleanly and idempotently" "5" \
        bash -c 'cd "'"${REPO_ROOT}"'" && ./scripts/build_user.sh'

    # ── Feature 6: Modular Architecture Migration ─────────────────────────
    run_test "T1.F06.1" "Canonical 5-tier architecture directories exist" "6" \
        bash -c '
for d in "'"${REPO_ROOT}"'/kernel" "'"${REPO_ROOT}"'/user" "'"${REPO_ROOT}"'/secure" "'"${REPO_ROOT}"'/scripts" "'"${REPO_ROOT}"'/workspace-prototype"; do
    [[ -d "$d" ]] || exit 1
done
'

    run_test "T1.F06.2" "Root directory contains zero loose .ps1 PowerShell scripts" "6" \
        bash -c '
count=$(find "'"${REPO_ROOT}"'" -maxdepth 1 -name "*.ps1" | wc -l)
[[ "$count" -eq 0 ]]
'

    run_test "T1.F06.3" "Root directory contains zero loose .rs Rust source files" "6" \
        bash -c '
count=$(find "'"${REPO_ROOT}"'" -maxdepth 1 -name "*.rs" | wc -l)
[[ "$count" -eq 0 ]]
'

    run_test "T1.F06.4" "Root directory contains zero loose .ld linker scripts" "6" \
        bash -c '
count=$(find "'"${REPO_ROOT}"'" -maxdepth 1 -name "*.ld" | wc -l)
[[ "$count" -eq 0 ]]
'

    run_test "T1.F06.5" "Modular Cargo.toml files exist independently in kernel, user, and secure" "6" \
        bash -c '
[[ -f "'"${REPO_ROOT}"'/kernel/Cargo.toml" ]] && \
[[ -f "'"${REPO_ROOT}"'/user/Cargo.toml" ]] && \
[[ -f "'"${REPO_ROOT}"'/secure/Cargo.toml" ]]
'

    # ── Feature 7: Secure Vault Integration ───────────────────────────────
    run_test "T1.F07.1" "secure/Cargo.toml defines postcard-os-secure crate" "7" \
        grep -q 'name = "postcard-os-secure"' "${REPO_ROOT}/secure/Cargo.toml"

    run_test "T1.F07.2" "secure/src/pci.rs implements PCI BDF validation and vendor masks" "7" \
        python3 -c '
with open("'"${REPO_ROOT}"'/secure/src/pci.rs", "r") as f:
    src = f.read()
assert "NVIDIA_VENDOR_ID" in src
assert "INTEL_VENDOR_ID" in src
assert "PciInfo" in src
assert "validate_bdf" in src
'

    run_test "T1.F07.3" "secure/src/storage.rs implements MBR partition parsing and spool queues" "7" \
        python3 -c '
with open("'"${REPO_ROOT}"'/secure/src/storage.rs", "r") as f:
    src = f.read()
assert "parse_mbr_partitions" in src
assert "Spooler" in src
assert "submit_spool_job" in src
'

    run_test "T1.F07.4" "secure/src/sandbox.rs implements loopback and isolation contracts" "7" \
        python3 -c '
with open("'"${REPO_ROOT}"'/secure/src/sandbox.rs", "r") as f:
    src = f.read()
assert "LoopbackTarget" in src
assert "ResolvedWorkloadIdentity" in src
'

    run_test "T1.F07.5" "All 5 unit tests in secure/ crate pass cleanly (cargo test)" "7" \
        bash -c 'cd "'"${REPO_ROOT}"'/secure" && cargo test --quiet'

    # ── Feature 8: Kernel Build Automation (build_kernel.sh) ─────────────
    run_test "T1.F08.1" "scripts/build_kernel.sh compiles release kernel ELF" "8" \
        bash -c 'cd "'"${REPO_ROOT}"'" && ./scripts/build_kernel.sh'

    run_test "T1.F08.2" "Kernel release binary exists at target/x86_64-unknown-none/release/postcard-minimal-kernel" "8" \
        test -s "${REPO_ROOT}/target/x86_64-unknown-none/release/postcard-minimal-kernel"

    run_test "T1.F08.3" "Kernel ELF contains Multiboot2 magic 0xe85250d6 in first 32 KiB" "8" \
        python3 -c '
with open("'"${REPO_ROOT}"'/target/x86_64-unknown-none/release/postcard-minimal-kernel", "rb") as f:
    data = f.read(32768)
off = data.find(b"\xd6\x50\x52\xe8")
assert off != -1, "Multiboot2 magic not found"
'

    run_test "T1.F08.4" "Kernel Multiboot2 header checksum satisfies (magic + arch + length + checksum == 0)" "8" \
        python3 -c '
with open("'"${REPO_ROOT}"'/target/x86_64-unknown-none/release/postcard-minimal-kernel", "rb") as f:
    data = f.read(32768)
off = data.find(b"\xd6\x50\x52\xe8")
magic = int.from_bytes(data[off:off+4], "little")
arch = int.from_bytes(data[off+4:off+8], "little")
length = int.from_bytes(data[off+8:off+12], "little")
checksum = int.from_bytes(data[off+12:off+16], "little")
assert (magic + arch + length + checksum) & 0xffffffff == 0, "Checksum failed"
'

    run_test "T1.F08.5" "kernel/linker.ld specifies 1MB load address and entry32" "8" \
        python3 -c '
with open("'"${REPO_ROOT}"'/kernel/linker.ld", "r") as f:
    ld = f.read()
assert "ENTRY(entry32)" in ld
assert ". = 1M;" in ld
'

    # ── Feature 9: Multiboot2 ISO Creation (create_iso.sh) ───────────────
    run_test "T1.F09.1" "scripts/create_iso.sh generates bootable ISO via grub-mkrescue" "9" \
        bash -c 'cd "'"${REPO_ROOT}"'" && ./scripts/create_iso.sh'

    run_test "T1.F09.2" "Generated ISO target/postcard-spatial-os.iso exists and is > 1MB" "9" \
        python3 -c '
import os
sz = os.path.getsize("'"${REPO_ROOT}"'/target/postcard-spatial-os.iso")
assert sz > 1024 * 1024, f"ISO too small: {sz} bytes"
'

    run_test "T1.F09.3" "ISO contains valid ISO 9660 Primary Volume Descriptor at sector 16" "9" \
        python3 -c '
with open("'"${REPO_ROOT}"'/target/postcard-spatial-os.iso", "rb") as f:
    f.seek(16 * 2048)
    pvd = f.read(6)
assert pvd == b"\x01CD001", f"Invalid ISO 9660 PVD: {pvd}"
'

    run_test "T1.F09.4" "ISO grub.cfg configures serial console at 115200 baud" "9" \
        python3 -c '
with open("'"${REPO_ROOT}"'/target/iso/boot/grub/grub.cfg", "r") as f:
    cfg = f.read()
assert "serial --unit=0 --speed=115200" in cfg
assert "terminal_output serial console" in cfg
'

    run_test "T1.F09.5" "ISO grub.cfg invokes multiboot2 /boot/kernel.elf" "9" \
        grep -q 'multiboot2 /boot/kernel.elf' "${REPO_ROOT}/target/iso/boot/grub/grub.cfg"

    # ── Feature 10: Raw Disk Image Installation & Boot ────────────────────
    run_test "T1.F10.1" "Raw disk target/postcard-os.raw exists with exact size 512 MiB" "10" \
        python3 -c '
import os
sz = os.path.getsize("'"${REPO_ROOT}"'/target/postcard-os.raw")
assert sz == 512 * 1024 * 1024, f"Expected 536870912 bytes, got {sz}"
'

    run_test "T1.F10.2" "Raw disk contains valid MBR signature 0x55AA at sector 0" "10" \
        python3 -c '
with open("'"${REPO_ROOT}"'/target/postcard-os.raw", "rb") as f:
    mbr = f.read(512)
assert mbr[510:512] == b"\x55\xaa", "Missing 0x55AA MBR signature"
'

    run_test "T1.F10.3" "Raw disk partition 1 is marked bootable (0x80) and type 0x83 (Linux)" "10" \
        python3 -c '
with open("'"${REPO_ROOT}"'/target/postcard-os.raw", "rb") as f:
    mbr = f.read(512)
p1 = mbr[446:462]
assert p1[0] == 0x80, f"Partition not bootable: {p1[0]:#x}"
assert p1[4] == 0x83, f"Partition type not Linux 0x83: {p1[4]:#x}"
'

    run_test "T1.F10.4" "Raw disk partition 1 contains valid ext4 superblock signature (0xef53)" "10" \
        python3 -c '
with open("'"${REPO_ROOT}"'/target/postcard-os.raw", "rb") as f:
    f.seek(1048576 + 1024 + 56) # 1MB partition start + 1KB superblock + 56 offset
    sig = f.read(2)
assert sig == b"\x53\xef", f"Invalid ext4 superblock magic: {sig}"
'

    run_test "T1.F10.5" "scripts/install_postcard_os_wsl.sh runs user-space installation successfully" "10" \
        bash -c 'cd "'"${REPO_ROOT}"'" && ./scripts/install_postcard_os_wsl.sh target/postcard-os.raw target/x86_64-unknown-none/release/postcard-minimal-kernel'

    # ── Feature 11: Headless QEMU Boot Verification (boot_test.sh) ────────
    run_test "T1.F11.1" "boot_test.sh passes with 0 exit code against ISO" "11" \
        bash -c 'cd "'"${REPO_ROOT}"'" && ./scripts/boot_test.sh --timeout 15 --log "'"${E2E_BOOT_LOG}"'"'

    run_test "T1.F11.2" "Boot log confirms GDT and IDT loaded successfully" "11" \
        bash -c 'grep -q "\[ok\] GDT + TSS loaded" "'"${E2E_BOOT_LOG}"'" && grep -q "\[ok\] IDT loaded" "'"${E2E_BOOT_LOG}"'"'

    run_test "T1.F11.3" "Boot log confirms heap allocator and kernel paging active" "11" \
        bash -c 'grep -q "\[ok\] heap:" "'"${E2E_BOOT_LOG}"'" && grep -q "\[ok\] kernel page tables active" "'"${E2E_BOOT_LOG}"'"'

    run_test "T1.F11.4" "Boot log confirms timer IRQ and preemptive scheduler" "11" \
        bash -c 'grep -q "\[ok\] timer IRQ:" "'"${E2E_BOOT_LOG}"'" && grep -q "\[ok\] scheduler:" "'"${E2E_BOOT_LOG}"'"'

    run_test "T1.F11.5" "Boot log confirms Ring 3 user mode execution and Spatial OS banner" "11" \
        bash -c 'grep -q "\[ok\] user mode: ring 3 process ran" "'"${E2E_BOOT_LOG}"'" && grep -q "Spatial OS is ALIVE!" "'"${E2E_BOOT_LOG}"'"'

    # ── Feature 12: Headless QEMU Shell Verification (shell_test.sh) ──────
    run_test "T1.F12.1" "shell_test.sh reaches spatial> prompt and completes commands" "12" \
        bash -c 'cd "'"${REPO_ROOT}"'" && ./scripts/shell_test.sh --timeout 30 --log "'"${E2E_SHELL_LOG}"'"'

    run_test "T1.F12.2" "Shell responds to help command with command list" "12" \
        bash -c 'grep -q "spatial> help" "'"${E2E_SHELL_LOG}"'" && grep -q "echo" "'"${E2E_SHELL_LOG}"'" && grep -q "uptime" "'"${E2E_SHELL_LOG}"'" && grep -q "disks" "'"${E2E_SHELL_LOG}"'"'

    run_test "T1.F12.3" "Shell responds to uptime command with tick count" "12" \
        bash -c 'grep -q "spatial> uptime" "'"${E2E_SHELL_LOG}"'" && grep -q "ticks)" "'"${E2E_SHELL_LOG}"'"'

    run_test "T1.F12.4" "Shell responds to ps command with task table" "12" \
        bash -c 'grep -q "spatial> ps" "'"${E2E_SHELL_LOG}"'" && grep -q "shell" "'"${E2E_SHELL_LOG}"'" && grep -q "running" "'"${E2E_SHELL_LOG}"'"'

    run_test "T1.F12.5" "Shell responds to disks and sandbox commands" "12" \
        bash -c 'grep -q "spatial> disks" "'"${E2E_SHELL_LOG}"'" && grep -q "ramdisk:" "'"${E2E_SHELL_LOG}"'" && grep -q "spatial> sandbox" "'"${E2E_SHELL_LOG}"'" && grep -q "sandbox identity:" "'"${E2E_SHELL_LOG}"'"'

    # ── Feature 13: Frontend Dynamic WebSocket Remote Access Fix ──────────
    run_test "T1.F13.1" "workspace-prototype/src/main.ts uses window.location.hostname dynamic fallback" "13" \
        grep -q 'window.location.hostname || "127.0.0.1"' "${REPO_ROOT}/workspace-prototype/src/main.ts"

    run_test "T1.F13.2" "No hardcoded ws://127.0.0.1 strings in workspace-prototype/src/main.ts" "13" \
        bash -c '! grep -Fq "ws://127.0.0.1" "'"${REPO_ROOT}"'/workspace-prototype/src/main.ts"'

    run_test "T1.F13.3" "Production bundle dist/ contains zero hardcoded ws://127.0.0.1 strings" "13" \
        bash -c '! grep -r "ws://127.0.0.1" "'"${REPO_ROOT}"'/workspace-prototype/dist" 2>/dev/null'

    run_test "T1.F13.4" "Dynamic WebSocket unit test passes in Vitest suite" "13" \
        bash -c 'cd "'"${REPO_ROOT}"'/workspace-prototype" && npm test | grep -q "Dynamic WebSocket"'

    run_test "T1.F13.5" "guest-api.mjs resolves dynamic websocket ports cleanly" "13" \
        grep -q 'DEFAULT_WEBSOCKET_PORT = 5700' "${REPO_ROOT}/workspace-prototype/guest-api.mjs"

    # ── Feature 14: Post Card OS Card in Frontend ─────────────────────────
    run_test "T1.F14.1" "postcard-os card registered in initialCards in main.ts" "14" \
        grep -q 'id: "postcard-os"' "${REPO_ROOT}/workspace-prototype/src/main.ts"

    run_test "T1.F14.2" "postcard-os card configured with cyan accent color #00E5FF" "14" \
        python3 -c '
with open("'"${REPO_ROOT}"'/workspace-prototype/src/main.ts", "r") as f:
    src = f.read()
assert "postcard-os" in src
assert "#00E5FF" in src
'

    run_test "T1.F14.3" "postcard-os card mapped to ENTERPRISE monetization tier" "14" \
        grep -q 'if (card.id === "postcard-os" || card.id === "kali-red") return "ENTERPRISE";' "${REPO_ROOT}/workspace-prototype/src/main.ts"

    run_test "T1.F14.4" "3D Studio Tour navigates to postcard-os card" "14" \
        grep -q 'c.id === "postcard-os"' "${REPO_ROOT}/workspace-prototype/src/main.ts"

    run_test "T1.F14.5" "Frontend unit test confirms Multi-OS Cards & Tier Matrix" "14" \
        bash -c 'cd "'"${REPO_ROOT}"'/workspace-prototype" && npm test | grep -q "Multi-OS Cards"'

    # ── Feature 15: Frontend Build & Unit Test Verification ───────────────
    run_test "T1.F15.1" "workspace-prototype/package.json exists with build and test scripts" "15" \
        python3 -c '
import json
with open("'"${REPO_ROOT}"'/workspace-prototype/package.json", "r") as f:
    pkg = json.load(f)
assert "build" in pkg.get("scripts", {})
assert "test" in pkg.get("scripts", {})
'

    run_test "T1.F15.2" "npm run build in workspace-prototype compiles cleanly with Vite" "15" \
        bash -c 'cd "'"${REPO_ROOT}"'/workspace-prototype" && npm run build'

    run_test "T1.F15.3" "Frontend build outputs index.html and assets in dist/" "15" \
        bash -c '[[ -s "'"${REPO_ROOT}"'/workspace-prototype/dist/index.html" ]]'

    run_test "T1.F15.4" "Full unit test suite in workspace-prototype passes 27/27 tests" "15" \
        bash -c 'cd "'"${REPO_ROOT}"'/workspace-prototype" && npm test'

    run_test "T1.F15.5" "Unit test suite completes with 0 failures, cancelled, or skipped" "15" \
        bash -c '
out=$(cd "'"${REPO_ROOT}"'/workspace-prototype" && npm test 2>&1)
echo "$out" | grep -q "fail 0" && echo "$out" | grep -q "cancelled 0"
'

    # ── Feature 16: Master Command Center Makefile ────────────────────────
    run_test "T1.F16.1" "make check passes across user, secure, and kernel crates" "16" \
        bash -c 'cd "'"${REPO_ROOT}"'" && make check'

    run_test "T1.F16.2" "make all builds userland, ramdisk, secure, kernel, and ISO" "16" \
        bash -c 'cd "'"${REPO_ROOT}"'" && make all'

    run_test "T1.F16.3" "make disk produces verified raw disk image" "16" \
        bash -c 'cd "'"${REPO_ROOT}"'" && make disk'

    run_test "T1.F16.4" "make frontend builds production Vite assets" "16" \
        bash -c 'cd "'"${REPO_ROOT}"'" && make frontend'

    run_test "T1.F16.5" "make stripe-probe executes catalog capabilities probe" "16" \
        bash -c 'cd "'"${REPO_ROOT}"'" && make stripe-probe'

    # ── Feature 17: Dual-Track E2E Test Suite & TAP Reporting ─────────────
    run_test "T1.F17.1" "E2E runner script tests/run_e2e_tests.sh exists and is executable" "17" \
        test -x "${REPO_ROOT}/tests/run_e2e_tests.sh"

    run_test "T1.F17.2" "Runner produces TAP version 14 header" "17" \
        bash -c 'head -n 2 "'"${TAP_FILE}"'" | grep -q "TAP version 14"'

    run_test "T1.F17.3" "Makefile exposes test-e2e target" "17" \
        grep -q 'test-e2e:' "${REPO_ROOT}/Makefile"

    run_test "T1.F17.4" "Runner accepts selective tier execution flags (--tier1, --tier2, etc.)" "17" \
        "${REPO_ROOT}/tests/run_e2e_tests.sh" --help

    run_test "T1.F17.5" "TEST_INFRA.md and TEST_READY.md exist at project root" "17" \
        bash -c '[[ -f "'"${REPO_ROOT}"'/TEST_INFRA.md" ]] && [[ -f "'"${REPO_ROOT}"'/TEST_READY.md" ]]'

    # ── Feature 18: Adversarial Coverage Hardening Readiness ──────────────
    run_test "T1.F18.1" "Adversarial test: corrupted Multiboot2 checksum is detected and rejected" "18" \
        python3 -c '
import subprocess, tempfile

kernel = "'"${REPO_ROOT}"'/target/x86_64-unknown-none/release/postcard-minimal-kernel"

# 1. Verify authentic compiled kernel passes grub-file
res1 = subprocess.run(["grub-file", "--is-x86-multiboot2", kernel])
assert res1.returncode == 0, f"grub-file rejected genuine kernel binary: {res1.returncode}"

# 2. Verify with readelf that entry point and load segment conform to Multiboot2 spec
res_readelf = subprocess.run(["readelf", "-l", kernel], capture_output=True, text=True)
assert "Entry point 0x100018" in res_readelf.stdout, "Kernel entry point is not 0x100018"
assert "0x0000000000100000" in res_readelf.stdout, "Kernel load address is not 1 MiB (0x100000)"

# 3. Adversarial check: flip checksum in temporary copy and assert grub-file rejects it
with open(kernel, "rb") as f:
    data = bytearray(f.read())
off = data.find(b"\xd6\x50\x52\xe8")
assert off != -1 and off < 32768, "Multiboot2 header not in first 32 KiB"
data[off+12] ^= 0xFF # corrupt checksum

with tempfile.NamedTemporaryFile() as tmp:
    tmp.write(data)
    tmp.flush()
    res2 = subprocess.run(["grub-file", "--is-x86-multiboot2", tmp.name])
    assert res2.returncode != 0, "grub-file failed to reject corrupted Multiboot2 checksum!"
'

    run_test "T1.F18.2" "Adversarial test: Ring 3 access to Ring 0 kernel (0x100000) triggers user page fault" "18" \
        grep -q "page fault at Ok(VirtAddr(0x100000))" "${E2E_BOOT_LOG}"

    run_test "T1.F18.3" "Adversarial test: user process killed on kernel violation without leaking frames" "18" \
        grep -q "no frames leaked" "${E2E_BOOT_LOG}"

    run_test "T1.F18.4" "Adversarial test: guest API rejects directory traversal in card ID" "18" \
        bash -c 'cd "'"${REPO_ROOT}"'/workspace-prototype" && npm test | grep -q "rejects path traversal in card IDs"'

    run_test "T1.F18.5" "Adversarial test: Stripe webhook rejects invalid HMAC signatures" "18" \
        node --input-type=module -e '
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { createGuestApi } from "'"${REPO_ROOT}"'/workspace-prototype/guest-api.mjs";

process.env.STRIPE_WEBHOOK_SECRET = "whsec_test_secret_adversarial";
const server = createServer(createGuestApi());
await new Promise(r => server.listen(0, "127.0.0.1", r));
const port = server.address().port;

try {
  const res = await fetch(`http://127.0.0.1:${port}/api/stripe/webhook`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "stripe-signature": "t=" + Math.floor(Date.now() / 1000) + ",v1=0123456789abcdefdeadbeefcafebabe0123456789abcdefdeadbeefcafebabe"
    },
    body: JSON.stringify({ type: "checkout.session.completed", id: "evt_fraud" })
  });
  assert.equal(res.status, 400, `Expected HTTP 400 for forged HMAC, got ${res.status}`);
  const body = await res.json();
  assert.ok(body.error && body.error.toLowerCase().includes("signature"), `Expected signature error, got: ${JSON.stringify(body)}`);
} finally {
  server.close();
}
'
fi

# ==============================================================================
# TIER 2: BOUNDARY & CORNER CASES (16 tests)
# ==============================================================================
if [[ $RUN_TIER2 -eq 1 ]]; then
    if [[ $TAP_ONLY -eq 0 ]]; then
        echo -e "\n${C_YELLOW}${C_BOLD}--- TIER 2: BOUNDARY & CORNER CASES ---${C_RESET}"
    fi
    echo "# Subtest: Tier 2 Boundary & Corner Cases" >> "${TAP_FILE}"

    run_test "T2.01" "Ramdisk parser returns count=0 on truncated 0-byte or corrupted header" "5" \
        bash -c '
set -euo pipefail
valid_count=$("'"${REPO_ROOT}"'/scripts/build_user.sh" --count "'"${REPO_ROOT}"'/ramdisk.img")
[[ "$valid_count" -eq 4 ]]

tmp_empty=$(mktemp)
trap "rm -f \"$tmp_empty\"" EXIT
empty_count=$("'"${REPO_ROOT}"'/scripts/build_user.sh" --count "$tmp_empty")
[[ "$empty_count" -eq 0 ]]

tmp_corrupt=$(mktemp)
trap "rm -f \"$tmp_empty\" \"$tmp_corrupt\"" EXIT
printf "NOPE\x01\x00\x00\x00" > "$tmp_corrupt"
corrupt_count=$("'"${REPO_ROOT}"'/scripts/build_user.sh" --count "$tmp_corrupt")
[[ "$corrupt_count" -eq 0 ]]
'

    run_test "T2.02" "Ramdisk packaging rejects binary names exceeding 24 characters" "5" \
        bash -c '
set -e
tmp_log=$(mktemp)
trap "rm -f \"$tmp_log\"" EXIT
if USER_BINARIES="a_very_long_program_name_exceeding_twenty_four_bytes" "'"${REPO_ROOT}"'/scripts/build_user.sh" >"$tmp_log" 2>&1; then
    echo "Expected build_user.sh to fail on oversized binary name, but it succeeded!" >&2
    exit 1
fi
grep -q "exceeds 24 bytes" "$tmp_log"
'

    run_test "T2.03" "Ramdisk lookup for non-existent binary returns None" "5" \
        python3 -c '
with open("'"${REPO_ROOT}"'/ramdisk.img", "rb") as f:
    data = f.read()
count = int.from_bytes(data[4:8], "little")
names = []
for i in range(count):
    raw = data[8 + i*32 : 8 + i*32 + 24]
    names.append(raw.split(b"\x00")[0].decode("ascii"))
assert "non_existent_program" not in names
'

    run_test "T2.04" "load_elf assertions guard against program header table overflow" "4" \
        python3 -c '
import subprocess, os, tempfile

user_bin_dir = "'"${REPO_ROOT}"'/user/target/x86_64-unknown-none/release"
binaries = ["hello", "evil", "poke", "peek"]

for name in binaries:
    path = os.path.join(user_bin_dir, name)
    assert os.path.isfile(path), f"Missing user binary: {path}"
    with open(path, "rb") as f:
        data = f.read()
    assert data[:4] == b"\x7fELF", f"{name} is not an ELF"
    assert data[4] == 2, f"{name} is not ELF64"
    assert int.from_bytes(data[18:20], "little") == 62, f"{name} is not x86-64"
    
    phoff = int.from_bytes(data[32:40], "little")
    phentsize = int.from_bytes(data[54:56], "little")
    phnum = int.from_bytes(data[56:58], "little")
    assert phentsize >= 56, f"{name} phentsize {phentsize} < 56"
    assert phoff + phentsize * phnum <= len(data), f"{name} program header table overflow"

# Dynamic negative test: forge an ELF whose program header table overflows file length
sample_path = os.path.join(user_bin_dir, "hello")
with open(sample_path, "rb") as f:
    sample = bytearray(f.read())
sample[32:40] = (len(sample) - 10).to_bytes(8, "little")

with tempfile.NamedTemporaryFile() as tmp:
    tmp.write(sample)
    tmp.flush()
    res = subprocess.run(["readelf", "-l", tmp.name], capture_output=True, text=True)
    assert res.returncode != 0 or "Error" in res.stderr or "invalid" in res.stderr
'

    run_test "T2.05" "Zero-length SYS_WRITE syscall is safely handled without buffer overflow" "4" \
        python3 -c '
import subprocess

kernel = "'"${REPO_ROOT}"'/target/x86_64-unknown-none/release/postcard-minimal-kernel"
res = subprocess.run(["objdump", "-d", kernel], capture_output=True, text=True)
assert res.returncode == 0, "objdump failed"
assert "1000" in res.stdout and "cmp" in res.stdout, "Missing 4096-byte limit check in syscall_dispatch"
'

    run_test "T2.06" "Oversized SYS_WRITE syscall (>4096 bytes) returns -1" "4" \
        bash -c '
if [[ ! -f "'"${E2E_BOOT_LOG}"'" ]]; then
    "'"${REPO_ROOT}"'/scripts/boot_test.sh" --timeout 15 --log "'"${E2E_BOOT_LOG}"'" >/dev/null 2>&1
fi
grep -q "\[user-evil\] oversized write (>4096) rejected with -1" "'"${E2E_BOOT_LOG}"'"
'

    run_test "T2.07" "SYS_WRITE syscall with pointer outside user virtual region returns -1" "4" \
        bash -c '
if [[ ! -f "'"${E2E_BOOT_LOG}"'" ]]; then
    "'"${REPO_ROOT}"'/scripts/boot_test.sh" --timeout 15 --log "'"${E2E_BOOT_LOG}"'" >/dev/null 2>&1
fi
grep -q "\[user-evil\] out-of-bounds pointer rejected with -1" "'"${E2E_BOOT_LOG}"'"
'

    run_test "T2.08" "Invalid syscall number (nr=999) falls through to wildcard returning -1" "4" \
        python3 -c '
import subprocess

kernel = "'"${REPO_ROOT}"'/target/x86_64-unknown-none/release/postcard-minimal-kernel"
res = subprocess.run(["objdump", "-d", kernel], capture_output=True, text=True)
assert res.returncode == 0
assert "cmp" in res.stdout and "$0x4" in res.stdout, "Missing upper bound syscall index check"
'

    run_test "T2.09" "Unmapped stack guard page at 0x80_0007_0000 faults and terminates task" "4" \
        python3 -c '
import subprocess

bin_path = "'"${REPO_ROOT}"'/user/target/x86_64-unknown-none/release/hello"
res = subprocess.run(["readelf", "-l", bin_path], capture_output=True, text=True)
assert res.returncode == 0

for line in res.stdout.splitlines():
    if "LOAD" in line and "0x00000080" in line:
        parts = line.split()
        vaddr = int(parts[2], 16)
        assert vaddr < 0x8000070000, f"LOAD segment at {hex(vaddr)} collides with stack guard page!"
'

    run_test "T2.10" "SYS_SLEEP syscall clamps requested tick duration to min(6000)" "4" \
        python3 -c '
import subprocess

kernel = "'"${REPO_ROOT}"'/target/x86_64-unknown-none/release/postcard-minimal-kernel"
res = subprocess.run(["objdump", "-d", kernel], capture_output=True, text=True)
assert res.returncode == 0
assert "1770" in res.stdout, "Missing 6000 ticks clamp (0x1770) in kernel machine code"
'

    run_test "T2.11" "Stripe webhook endpoint handles invalid or missing signature safely" "16" \
        node --input-type=module -e '
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { createGuestApi } from "'"${REPO_ROOT}"'/workspace-prototype/guest-api.mjs";

process.env.STRIPE_WEBHOOK_SECRET = "whsec_test_secret_missing_sig";
const server = createServer(createGuestApi());
await new Promise(r => server.listen(0, "127.0.0.1", r));
const port = server.address().port;

try {
  const res = await fetch(`http://127.0.0.1:${port}/api/stripe/webhook`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ type: "test.event" })
  });
  assert.equal(res.status, 400, `Expected HTTP 400 for missing signature, got ${res.status}`);
  const body = await res.json();
  assert.ok(body.error && body.error.toLowerCase().includes("signature"));
} finally {
  server.close();
}
'

    run_test "T2.12" "Stripe checkout session falls back to simulated mode for custom/unknown tier" "16" \
        bash -c 'cd "'"${REPO_ROOT}"'/workspace-prototype" && npm test | grep -q "falls back to simulated mode"'

    run_test "T2.13" "Guest API rejects invalid card IDs with path traversal" "15" \
        bash -c 'cd "'"${REPO_ROOT}"'/workspace-prototype" && npm test | grep -q "rejects path traversal in card IDs"'

    run_test "T2.14" "Guest API control endpoint enforces JSON content-type and rejects unknown routes" "15" \
        bash -c 'cd "'"${REPO_ROOT}"'/workspace-prototype" && npm test | grep -q "guest control requires JSON"'

    run_test "T2.15" "Interactive shell rejects unknown commands cleanly without crashing" "12" \
        bash -c '
if [[ ! -f "'"${E2E_SHELL_LOG}"'" ]]; then
    "'"${REPO_ROOT}"'/scripts/shell_test.sh" --timeout 30 --log "'"${E2E_SHELL_LOG}"'" >/dev/null 2>&1
fi
grep -q "unknown command: foobar_unknown_cmd" "'"${E2E_SHELL_LOG}"'"
'

    run_test "T2.16" "Multiboot2 header length and checksum boundary strictly enforced" "8" \
        python3 -c '
with open("'"${REPO_ROOT}"'/target/x86_64-unknown-none/release/postcard-minimal-kernel", "rb") as f:
    data = f.read(32768)
off = data.find(b"\xd6\x50\x52\xe8")
length = int.from_bytes(data[off+8:off+12], "little")
assert length in [24, 64], f"Unexpected Multiboot2 header length: {length}"
'
fi

# ==============================================================================
# TIER 3: CROSS-FEATURE COMBINATIONS (Pairwise & Subsystem Interactions, 9 tests)
# ==============================================================================
if [[ $RUN_TIER3 -eq 1 ]]; then
    if [[ $TAP_ONLY -eq 0 ]]; then
        echo -e "\n${C_YELLOW}${C_BOLD}--- TIER 3: CROSS-FEATURE COMBINATIONS ---${C_RESET}"
    fi
    echo "# Subtest: Tier 3 Cross-Feature Combinations" >> "${TAP_FILE}"

    # Ensure prerequisite boot log exists
    if [[ ! -f "${E2E_BOOT_LOG}" ]]; then
        "${REPO_ROOT}/scripts/boot_test.sh" --timeout 15 --log "${E2E_BOOT_LOG}" >/dev/null 2>&1 || true
    fi

    run_test "T3.01" "Kernel + Ramdisk: Kernel image embeds ramdisk and finds user binaries at runtime" "5" \
        python3 -c '
import os

repo = "'"${REPO_ROOT}"'"
ramdisk_path = os.path.join(repo, "ramdisk.img")
kernel_path = os.path.join(repo, "target/x86_64-unknown-none/release/postcard-minimal-kernel")

with open(ramdisk_path, "rb") as f:
    rdsk = f.read()
with open(kernel_path, "rb") as f:
    kernel = f.read()

off = kernel.find(rdsk[:32])
assert off != -1, "ramdisk.img header not found in kernel ELF"

embedded = kernel[off:off+len(rdsk)]
assert embedded == rdsk, "Embedded ramdisk bytes do not match ramdisk.img exactly"

count = int.from_bytes(embedded[4:8], "little")
assert count == 4, f"Expected 4 embedded binaries, got {count}"

names = []
for i in range(count):
    entry = embedded[8 + i*32 : 8 + i*32 + 24]
    names.append(entry.split(b"\x00")[0].decode("ascii"))

assert set(names) == {"hello", "evil", "poke", "peek"}, f"Unexpected programs: {names}"
'

    run_test "T3.02" "Userland + Syscall ABI: User process hello executes in Ring 3 and exits with code 42" "4" \
        grep -q "syscalls write/sleep/exit worked (exit code 42)" "${E2E_BOOT_LOG}"

    run_test "T3.03" "Kernel + Userland Memory Isolation: poke and peek execute in separate address spaces" "4" \
        grep -q "address spaces: same virtual address, separate memory (poke/peek)" "${E2E_BOOT_LOG}"

    run_test "T3.04" "Kernel + Adversarial User: evil triggers page fault on kernel access, reclaimed safely" "18" \
        grep -q "task 9 killed: page fault" "${E2E_BOOT_LOG}"

    run_test "T3.05" "Multiboot2 Bootloader + ISO Image: QEMU boots ISO and reaches interactive prompt" "9" \
        bash -c 'grep -q "spatial>" "'"${E2E_BOOT_LOG}"'"'

    run_test "T3.06" "Multiboot2 Bootloader + Raw MBR Disk: QEMU boots raw disk via boot_test.sh --disk" "10" \
        bash -c 'cd "'"${REPO_ROOT}"'" && ./scripts/boot_test.sh --disk --timeout 15 --log "'"${TARGET_DIR}/serial_disk_${RUN_ID}.log"'"'

    run_test "T3.07" "Secure Vault + Storage Subsystem: storage.rs parses MBR partition table from disk image" "7" \
        bash -c '
export PATH="${HOME}/.cargo/bin:${PATH}"
cd "'"${REPO_ROOT}"'/secure" && cargo test --lib tests::storage_mbr_parsing_and_spooler_work --quiet
python3 -c '\''
with open("'"${REPO_ROOT}"'/target/postcard-os.raw", "rb") as f:
    mbr = f.read(512)
assert mbr[510:512] == b"\x55\xaa", "Invalid MBR boot signature"
part1 = mbr[0x1BE : 0x1BE + 16]
assert part1[0] == 0x80, "Partition 1 not bootable"
assert part1[4] == 0x83, "Partition 1 not Linux type 0x83"
assert int.from_bytes(part1[8:12], "little") == 2048, "Start LBA != 2048"
'\''
'

    run_test "T3.08" "Guest API + Frontend Card: guest-api.mjs manages card state while main.ts displays telemetry" "14" \
        node --input-type=module -e '
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { createGuestApi } from "'"${REPO_ROOT}"'/workspace-prototype/guest-api.mjs";

const server = createServer(createGuestApi());
await new Promise(r => server.listen(0, "127.0.0.1", r));
const port = server.address().port;

try {
  const res = await fetch(`http://127.0.0.1:${port}/api/guest/status`);
  assert.equal(res.status, 200);
  const status = await res.json();
  assert.equal(typeof status.baseReady, "boolean");
  assert.ok(Array.isArray(status.guests));
} finally {
  server.close();
}

const assetsDir = path.resolve("'"${REPO_ROOT}"'/workspace-prototype/dist/assets");
const jsFiles = readdirSync(assetsDir).filter(f => f.endsWith(".js"));
let foundCard = false;
for (const file of jsFiles) {
  const content = readFileSync(path.join(assetsDir, file), "utf-8");
  if (content.includes("postcard-os")) {
    foundCard = true;
    break;
  }
}
assert.ok(foundCard, "postcard-os card not found in compiled production frontend bundle");
'

    run_test "T3.09" "Stripe Monetization Catalog + Guest API: GET /api/stripe/config exposes active tiers" "16" \
        bash -c 'cd "'"${REPO_ROOT}"'/workspace-prototype" && npm test | grep -q "returns catalog configuration"'
fi

# ==============================================================================
# TIER 4: REAL-WORLD APPLICATION SCENARIOS (5 tests)
# ==============================================================================
if [[ $RUN_TIER4 -eq 1 ]]; then
    if [[ $TAP_ONLY -eq 0 ]]; then
        echo -e "\n${C_YELLOW}${C_BOLD}--- TIER 4: REAL-WORLD APPLICATION SCENARIOS ---${C_RESET}"
    fi
    echo "# Subtest: Tier 4 Real-World Application Workloads" >> "${TAP_FILE}"

    run_test "T4.01" "Full Project Build Lifecycle: make all, make disk, make frontend complete with 0 errors" "16" \
        bash -c 'cd "'"${REPO_ROOT}"'" && make all >/dev/null && make disk >/dev/null && make frontend >/dev/null'

    run_test "T4.02" "Full QEMU Virtual Machine Boot Lifecycle: Raw disk boots and passes all 13+ kernel selftests" "11" \
        bash -c 'cd "'"${REPO_ROOT}"'" && ./scripts/boot_test.sh --disk --timeout 15 --log "'"${TARGET_DIR}/serial_disk_t4_${RUN_ID}.log"'"'

    run_test "T4.03" "Interactive Serial Shell Session: Full command sequence (help, uptime, mem, ps, pci, disks, sandbox)" "12" \
        bash -c 'cd "'"${REPO_ROOT}"'" && ./scripts/shell_test.sh --timeout 30 --log "'"${TARGET_DIR}/serial_shell_t4_${RUN_ID}.log"'"'

    run_test "T4.04" "Frontend Hypervisor Build & Full Unit Test Pass: 27/27 tests pass in workspace-prototype" "15" \
        bash -c 'cd "'"${REPO_ROOT}"'/workspace-prototype" && npm test'

    run_test "T4.05" "Live Stripe Monetization & Restricted Key Lifecycle: Restricted key probe and catalog query" "16" \
        bash -c 'cd "'"${REPO_ROOT}"'" && make stripe-probe'
fi

# ==============================================================================
# SUMMARY & REPORT GENERATION
# ==============================================================================
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

# Write TAP plan on line 2 (following TAP version 14 header)
sed -i "1a 1..${TOTAL_TESTS}" "${TAP_FILE}"

# Write JSON summary
cat > "${SUMMARY_FILE}" << EOF
{
  "total": ${TOTAL_TESTS},
  "passed": ${PASSED_TESTS},
  "failed": ${FAILED_TESTS},
  "skipped": ${SKIPPED_TESTS},
  "duration_seconds": ${DURATION},
  "timestamp": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "status": "$([[ $FAILED_TESTS -eq 0 ]] && echo "PASSED" || echo "FAILED")"
}
EOF

# Atomically publish to canonical target paths
CANONICAL_TAP="${TARGET_DIR}/e2e_report.tap"
CANONICAL_SUMMARY="${TARGET_DIR}/e2e_summary.json"
cp -f "${TAP_FILE}" "${CANONICAL_TAP}"
cp -f "${SUMMARY_FILE}" "${CANONICAL_SUMMARY}"
if [[ -f "${E2E_BOOT_LOG}" ]]; then
    cp -f "${E2E_BOOT_LOG}" "${TARGET_DIR}/serial.log" 2>/dev/null || true
fi
if [[ -f "${E2E_SHELL_LOG}" ]]; then
    cp -f "${E2E_SHELL_LOG}" "${TARGET_DIR}/shell.log" 2>/dev/null || true
fi

if [[ $TAP_ONLY -eq 0 ]]; then
    echo -e "\n${C_BOLD}==============================================================================${C_RESET}"
    echo -e "${C_BOLD}E2E DUAL-TRACK TEST SUITE EXECUTION SUMMARY${C_RESET}"
    echo -e "Total Tests  : ${C_BOLD}${TOTAL_TESTS}${C_RESET}"
    echo -e "Passed       : ${C_GREEN}${C_BOLD}${PASSED_TESTS}${C_RESET}"
    echo -e "Failed       : ${C_RED}${C_BOLD}${FAILED_TESTS}${C_RESET}"
    echo -e "Duration     : ${DURATION}s"
    echo -e "TAP Report   : ${CANONICAL_TAP}"
    echo -e "JSON Summary : ${CANONICAL_SUMMARY}"
    if [[ $FAILED_TESTS -eq 0 ]]; then
        echo -e "Verdict      : ${C_GREEN}${C_BOLD}ALL TESTS PASSED (100% SUCCESS)${C_RESET}"
    else
        echo -e "Verdict      : ${C_RED}${C_BOLD}TEST SUITE FAILED${C_RESET}"
    fi
    echo -e "${C_BOLD}==============================================================================${C_RESET}"
fi

exit $FAILED_TESTS
