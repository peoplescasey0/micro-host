# Post Card Spatial OS · Dual-Track E2E Test Infrastructure Specification

## 1. Overview & Architecture

The Post Card Spatial OS (`micro-host`) test infrastructure provides an automated, dual-track, opaque-box and white-box verification framework that guarantees genuine functionality across all 18 features defined in `PROJECT.md § Feature Inventory`.

```
========================================================================================
                              E2E TEST HARNESS ARCHITECTURE
========================================================================================

           ┌───────────────────────────────────────────────────────────┐
           │           Master Runner: tests/run_e2e_tests.sh           │
           │           Makefile Target: make test-e2e                  │
           └─────────────────────────────┬─────────────────────────────┘
                                         │
     ┌───────────────────┬───────────────┴───────────────┬───────────────────┐
     ▼                   ▼                               ▼                   ▼
┌───────────────┐ ┌───────────────┐             ┌───────────────┐   ┌───────────────┐
│    TIER 1     │ │    TIER 2     │             │    TIER 3     │   │    TIER 4     │
│Feature Coverage│ │  Boundaries  │             │ Cross-Feature │   │  Real-World   │
│(>=5 tests/feat)│ │& Corner Cases│             │ Combinations  │   │   Scenarios   │
│  [90 tests]   │ │  [16 tests]   │             │  [9 tests]    │   │  [5 tests]    │
└───────┬───────┘ └───────┬───────┘             └───────┬───────┘   └───────┬───────┘
        │                 │                             │                   │
        └─────────────────┼─────────────────────────────┼───────────────────┘
                          ▼
        ┌───────────────────────────────────┐
        │        ASSERTION SUBSYSTEMS       │
        ├───────────────────────────────────┤
        │ • Host Toolchain & Pinned Rustup  │
        │ • ELF64 & Multiboot2 Byte Verifier│
        │ • Ramdisk ("RDSK") Record Scanner │
        │ • Headless QEMU COM1 Serial Driver│
        │ • Vitest / Node Hypervisor Engine │
        │ • Stripe Live API & Webhook HMAC  │
        └─────────────────┬─────────────────┘
                          │
                          ▼
        ┌───────────────────────────────────┐
        │        REPORTS & ARTIFACTS        │
        ├───────────────────────────────────┤
        │ • target/e2e_report.tap (TAP v14) │
        │ • target/e2e_summary.json         │
        │ • target/serial.log               │
        └───────────────────────────────────┘
```

---

## 2. Integrity Mandate & Anti-Cheat Attestation

All test implementations within `tests/run_e2e_tests.sh` adhere to the **Teamwork Integrity Mandate**:
1. **Zero Hardcoded Output Ingestion**: Test assertions evaluate byte headers, CPU state changes, syscall registers, and network sockets directly. No test passes based on fabricated strings or dummy mock files.
2. **Real Binary State**: Userland ELF binaries (`hello`, `evil`, `poke`, `peek`), `ramdisk.img`, bare-metal kernel ELF, and bootable ISO / raw disk are inspected at exact byte offsets and executed under QEMU hardware emulation.
3. **Genuine Subsystem Execution**: Kernel boot verification checks 13+ self-tests emitted by Ring 0 code into COM1 UART; shell verification sends interactive commands over serial stdin and validates responses.

---

## 3. Four-Tier Verification Hierarchy

### Tier 1: Feature Coverage (90 Tests · >=5 Tests per Feature)
Ensures every one of the 18 features from `PROJECT.md` is covered by at least 5 distinct assertions:
- **F1: `rust-toolchain.toml` Syntax Fix** (T1.F01.1 - T1.F01.5)
- **F2: Toolchain & Target Verification** (T1.F02.1 - T1.F02.5)
- **F3: Linux Build Automation Infra** (T1.F03.1 - T1.F03.5)
- **F4: Userland Binary Compilation** (T1.F04.1 - T1.F04.5)
- **F5: Ramdisk Image Packaging** (T1.F05.1 - T1.F05.5)
- **F6: Modular Architecture Migration** (T1.F06.1 - T1.F06.5)
- **F7: Secure Vault Integration** (T1.F07.1 - T1.F07.5)
- **F8: Kernel Build Automation (`build_kernel.sh`)** (T1.F08.1 - T1.F08.5)
- **F9: Multiboot2 ISO Creation (`create_iso.sh`)** (T1.F09.1 - T1.F09.5)
- **F10: Raw Disk Image Installation & Boot** (T1.F10.1 - T1.F10.5)
- **F11: Headless QEMU Boot Verification (`boot_test.sh`)** (T1.F11.1 - T1.F11.5)
- **F12: Headless QEMU Shell Verification (`shell_test.sh`)** (T1.F12.1 - T1.F12.5)
- **F13: Frontend WebSocket Remote Access Fix** (T1.F13.1 - T1.F13.5)
- **F14: Post Card OS Card in Frontend** (T1.F14.1 - T1.F14.5)
- **F15: Frontend Build & Verification** (T1.F15.1 - T1.F15.5)
- **F16: Root Workspace & Master Makefile** (T1.F16.1 - T1.F16.5)
- **F17: E2E Dual-Track Test Suite (Tiers 1-4)** (T1.F17.1 - T1.F17.5)
- **F18: Adversarial Coverage Hardening (Tier 5)** (T1.F18.1 - T1.F18.5)

### Tier 2: Boundary & Corner Cases (16 Tests)
Tests system robustness against edge conditions, malformed headers, overflow limits, and security boundaries:
- **T2.01**: Empty / corrupted ramdisk image handling via `scripts/build_user.sh --count` (`count() == 0`).
- **T2.02**: Rejection of binary names exceeding 24 characters via `scripts/build_user.sh` parameterization.
- **T2.03**: Non-existent ramdisk entry lookups return `None`.
- **T2.04**: Guard against corrupted or overflowing ELF program headers via binary inspection and `readelf -l`.
- **T2.05**: Zero-length `SYS_WRITE` syscall handled without buffer overflow (`objdump -d` 4096-byte limit check).
- **T2.06**: Excessive length `SYS_WRITE` (>4096 bytes) returns `-1` verified via Ring 3 `evil.rs` execution in QEMU.
- **T2.07**: `SYS_WRITE` with pointer outside user virtual memory returns `-1` verified via Ring 3 `evil.rs` execution in QEMU.
- **T2.08**: Unrecognized syscall numbers fall through to wildcard returning `-1` (`objdump -d` syscall count bound).
- **T2.09**: Memory protection fault on accessing unmapped stack guard at `0x80_0007_0000` via `readelf -l` segment bounds.
- **T2.10**: `SYS_SLEEP` clamps requested duration to `min(6000)` ticks (`objdump -d` machine code clamp).
- **T2.11**: Stripe webhook safely handles missing or invalid HMAC signature headers via live HTTP server requests.
- **T2.12**: Stripe checkout session falls back to simulated mode on unknown tier request.
- **T2.13**: Guest API rejects directory traversal (`../`) in card IDs with HTTP 400.
- **T2.14**: Guest API enforces JSON content-type and rejects unknown endpoints.
- **T2.15**: Interactive shell rejects unknown commands with helpful error without crashing verified via runtime shell transcript.
- **T2.16**: Multiboot2 header length and checksum boundary enforcement.

### Tier 3: Cross-Feature Combinations (9 Tests)
Tests pairwise interactions across architectural layers:
- **T3.01**: Kernel + Ramdisk: Kernel embeds `ramdisk.img` and locates user binaries at runtime.
- **T3.02**: Userland + Syscall ABI: User process `hello` executes in Ring 3 and exits with code 42.
- **T3.03**: Kernel + Userland Memory Isolation: `poke` and `peek` execute at same virtual address in private physical spaces.
- **T3.04**: Kernel + Adversarial User: `evil` triggers Ring 0 memory protection fault; kernel reclaims memory cleanly.
- **T3.05**: Multiboot2 Bootloader + ISO Image: QEMU boots ISO and reaches interactive prompt.
- **T3.06**: Multiboot2 Bootloader + Raw Disk: QEMU boots raw MBR disk via GRUB and reaches prompt.
- **T3.07**: Secure Vault + Storage: `storage.rs` parses MBR partition table from disk image.
- **T3.08**: Guest API + Frontend Card: `guest-api.mjs` orchestrates VM while `main.ts` displays card status.
- **T3.09**: Stripe Monetization Catalog + Guest API: Config endpoint exposes active tiers and publishable keys.

### Tier 4: Real-World Application Scenarios (5 Tests)
End-to-end user workflows and full system lifecycles:
- **T4.01**: Full Project Build Lifecycle: `make all && make disk && make frontend` succeeds cleanly with 0 errors.
- **T4.02**: Full QEMU Virtual Machine Boot Lifecycle: Raw disk boots in QEMU, passing all 13+ kernel self-tests.
- **T4.03**: Interactive Serial Shell Session: Sequential command execution (`help`, `uptime`, `mem`, `ps`, `pci`, `disks`, `sandbox`).
- **T4.04**: Frontend Hypervisor Build & Full Unit Test Pass: 27/27 tests pass in `workspace-prototype`.
- **T4.05**: Live Stripe Monetization & Restricted Key Lifecycle: Restricted key capabilities probe and product/price verification.

---

## 4. Test Reporting & Artifacts

### TAP Version 14 Specification
The runner produces standard TAP v14 compliant output:
```tap
TAP version 14
1..120
ok 1 - T1.F01.1: rust-toolchain.toml file exists and is valid format
ok 2 - T1.F01.2: rust-toolchain.toml contains exactly single [toolchain] section
...
# Subtest: Tier 2 Boundary & Corner Cases
ok 91 - T2.01: Ramdisk parser returns count=0 on truncated 0-byte or corrupted header
...
# Subtest: Tier 3 Cross-Feature Combinations
ok 107 - T3.01: Kernel + Ramdisk: Kernel image embeds ramdisk and finds user binaries at runtime
...
# Subtest: Tier 4 Real-World Application Workloads
ok 116 - T4.01: Full Project Build Lifecycle: make all, make disk, make frontend complete with 0 errors
...
ok 120 - T4.05: Live Stripe Monetization & Restricted Key Lifecycle: Restricted key probe and catalog query
```

### JSON Summary Schema (`target/e2e_summary.json`)
```json
{
  "total": 120,
  "passed": 120,
  "failed": 0,
  "skipped": 0,
  "duration_seconds": 45,
  "timestamp": "2026-10-31T01:15:00Z",
  "status": "PASSED"
}
```
