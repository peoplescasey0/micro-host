# Post Card OS (`micro-host`) — Workspace Rules

## Architecture & Repository Layout

This repository contains two primary components plus supporting backend services:
1. **Bare-Metal Kernel (`src/`, `user/`, `Cargo.toml`, `linker.ld`)**:
   - Target: `x86_64-unknown-none` using pinned Rust toolchain `nightly-2024-12-01`.
   - Freestanding `#![no_std]` and `#![no_main]` environment with `panic = "abort"`.
   - Subsystems: UART 16550 serial (`COM1` at `0x3F8`), GDT/TSS with dedicated double-fault IST stack, IDT exception/interrupt handlers, physical frame allocator (`BootInfoFrameAllocator`), paging, heap allocator (`BumpAllocator` / `LockedHeap`), task context switching, and ring-3 user-mode demo programs (`user/`).
2. **Spatial Workspace Prototype (`workspace-prototype/`)**:
   - Host-side 3D spatial workspace built with TypeScript, Three.js, Vite, and `@novnc/novnc`.
   - Includes `guest-api.mjs` for managing live QEMU guests (Debian Linux with shared golden base + per-card QCOW2 overlays, and Windows 11 via WSL 2 QEMU + `swtpm` + OVMF Secure Boot).
3. **Backend & Database (`secure_backend/`, `supabase/`)**:
   - Supabase configuration, migrations, and backend services. Always consult the `supabase` and `supabase-postgres-best-practices` skills before modifying database schemas, RLS policies, or queries.

## Strict Engineering Guidelines

### 1. Rust Bare-Metal Kernel (`src/` & `user/`)
- **No Standard Library**: Never import `std` in kernel or user-space bare-metal crates. Use `core` and `alloc` (when heap is initialized).
- **Interrupt & Spinlock Safety**: Never hold a `spin::Mutex` lock across an operation that can trigger a synchronous exception or deadlock with an IRQ handler (e.g., `SERIAL1`, keyboard ring buffer, or scheduler locks). Disable interrupts (`x86_64::instructions::interrupts::without_interrupts`) when accessing shared state mutated by hardware IRQs.
- **Unsafe Code Discipline**: Minimize `unsafe` blocks. Every `unsafe` block must uphold clear invariants (valid physical/virtual addresses, ABI compliance, port I/O permissions, or page-table ownership).
- **Documentation Honesty**: Do not claim SMP, general device management, spooling, partition management, or writable kernel filesystems are complete until their implementation and acceptance tests exist and pass.

### 2. QEMU, Disk Images, and Guest Safety
- **Loopback Binding Only**: VNC WebSocket (`5700+`), QMP (`4444+`), and `swtpm` ports must always bind strictly to `127.0.0.1` (loopback). Never expose guest control or display ports to external network interfaces.
- **Protect Persistent VM State**:
  - Do not overwrite or delete `target/postcard-os.raw` unless explicitly requested by the user.
  - Do not delete or corrupt `workspace-prototype/data/master-images/debian-golden.qcow2` or finalized per-card overlays in `workspace-prototype/data/overlays/`.
  - Never delete `workspace-prototype/data/windows/` TPM 2.0 state or UEFI NVRAM (`OVMF_VARS`), as doing so permanently invalidates the guest's TPM-bound Windows installation.

### 3. Build & Verification Commands
- **Root Workspace scripts (`package.json`)**:
  - `npm run dev`, `npm run build`, `npm run preview`, and `npm test` delegate to `workspace-prototype`.
- **Kernel verification**:
  - Use `cargo check` / `cargo build` with the pinned nightly toolchain and run `.\boot_test.ps1` or `.\shell_test.ps1` to verify serial output via QEMU.
- **Workspace Prototype verification**:
  - Run `npm test` (`node --test` in `workspace-prototype`) and `npm run build` (`tsc --noEmit && vite build`).

