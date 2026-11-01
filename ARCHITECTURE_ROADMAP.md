# Post Card Spatial OS · Architecture Roadmap & System Specifications
**Repository**: `/home/casey/micro-host/micro-host`  
**Milestone**: M6 (System Architecture, Storage Driver, Golden Images & Monetization)  
**Status**: ACTIVE REFERENCE SPECIFICATION  

---

## 1. The Canonical 5-Tier Architecture

The Post Card Spatial OS (`micro-host`) is strictly organized into five canonical tiers, ensuring total separation of concerns between bare-metal hardware, isolated userland, hardware virtualization vault, spatial hypervisor, and build automation:

```
micro-host/
├── kernel/                 # 🟢 Tier 1: The Core Metal (Ring 0 Microkernel)
│   ├── src/                # Paging, scheduler, IDT, GDT, usermode, PCI, storage, shell
│   ├── Cargo.toml          # x86_64, spin, uart_16550, linked_list_allocator
│   └── linker.ld           # Multiboot2 ELF layout (1 MiB load, 32-bit entry, 64-bit bootstrap)
│
├── user/                   # 🟢 Tier 2: The Userland Gremlins (Ring 3 ABI)
│   ├── src/bin/            # hello.rs, evil.rs, poke.rs, peek.rs
│   ├── user.ld             # User virtual base (0x80_0000_0000)
│   └── Cargo.toml          # Standalone #![no_std] user crate
│
├── secure/                 # 🟢 Tier 3: The Vault (Security, Passthrough & Sandboxing)
│   ├── src/                # pci.rs, storage.rs, sandbox.rs, vfio.rs, opensandbox_bridge.rs
│   └── Cargo.toml          # serde, sha3, thiserror
│
├── workspace-prototype/    # 🟢 Tier 4: The Spatial Hypervisor (Frontend & Control Plane)
│   ├── src/                # Three.js 3D spatial studio, noVNC RFB client, UI logic
│   ├── guest-api.mjs       # QEMU / QMP / VNC / Stripe orchestration backend
│   └── package.json        # Vite, Three.js, @novnc/novnc
│
├── scripts/                # 🟢 Tier 5: The Automation Engine (Build & Test Infrastructure)
│   ├── build_user.sh       # Compiles userland & packs ramdisk.img
│   ├── build_kernel.sh     # Compiles bare-metal kernel ELF
│   ├── create_iso.sh       # Generates Multiboot2 ISO via grub-mkrescue
│   ├── boot_test.sh        # Headless QEMU verification runner
│   ├── shell_test.sh       # Headless QEMU serial shell test runner
│   ├── install_postcard_os_wsl.sh # User-space MBR partitioner and GRUB installer
│   └── run_postcard_os.sh  # Interactive QEMU launcher with serial console
│
└── Makefile                # 🟢 Master Command Center (Root Orchestration)
```

### 1.1 Tier 1: Ring 0 Microkernel (`kernel/`)
- **Multiboot2 Bootstrap**: Loads at physical address `0x00100000` (1 MiB). The Multiboot2 header (`0xe85250d6`) is embedded in `.text.boot` within the first 32 KiB.
- **Entry & Mode Switch**: GRUB transfers control at `entry32`. The bootstrap sets up identity paging for the first 1 GiB, enables `CR4.PAE`, toggles `IA32_EFER.LME` for x86_64 Long Mode, loads the 64-bit GDT (`boot_gdt_ptr`), enables paging (`CR0.PG`), and long jumps to 64-bit `long_mode_start`.
- **Memory Protection**:
  - Enforces strict `W^X` permissions: `.text` is Read+Execute (RX), `.rodata` is Read-Only (R), `.data` and `.bss` are Read-Write+NoExecute (RW+NX).
  - Null page (`0x0000_0000`) is unmapped to fault on null pointer dereferences.
  - Kernel stack is protected by a 4 KiB unmapped guard page.
- **Preemptive Scheduler**: PIT timer interrupt configured at 100 Hz (10 ms timeslice). Round-robin scheduling switches thread contexts (`switch_context(prev_rsp, next_rsp)`).
- **Serial & VGA Shell**: COM1 UART 16550 (`0x3F8`) and VGA text buffer (`0xB8000`) support non-blocking interactive command execution (`help`, `uptime`, `mem`, `ps`, `pci`, `disks`, `sandbox`).

### 1.2 Tier 2: Ring 3 Userland ABI (`user/`)
- **Virtual Memory Layout**: User executables link at base `0x80_0000_0000` (PML4 slot 1). The 16 KiB user stack grows downward from `0x80_0007_5000` and is protected by a 4 KiB unmapped guard page at `0x80_0007_0000`.
- **Fast System Calls (`syscall` / `sysretq`)**:
  - `IA32_STAR` (`0xC0000081`): Configures kernel and user CS/SS segment selectors.
  - `IA32_LSTAR` (`0xC0000082`): Points to `syscall_entry` dispatcher.
  - `IA32_FMASK` (`0xC0000084`): Clears Interrupt Flag (`0x200`) on entry.
- **Syscall Calling Convention**:
  - `RAX` = Syscall Number, `RDI` = Arg 1, `RSI` = Arg 2, `RDX` = Arg 3. Returns signed 64-bit integer in `RAX`.
  - `0`: `SYS_WRITE(buf, len)` — Validates user space bounds (`>= 0x80_0000_0000`); kernel pointers return `-14` (`EFAULT`).
  - `1`: `SYS_EXIT(code)` — Terminates process, wakes waiting tasks, frees address space frames.
  - `2`: `SYS_GETPID()` — Returns current task ID.
  - `3`: `SYS_SLEEP(ticks)` — Puts task to sleep for specified timer ticks.
  - `4`: `SYS_YIELD()` — Yields remaining timeslice.
- **Binaries**:
  - `hello.rs`: Prints banner, sleeps, and exits with code 42.
  - `evil.rs`: Attempts unauthorized read of kernel memory (`0x0010_0000`); page fault handler terminates process with zero memory leakage.
  - `poke.rs` & `peek.rs`: Concurrently verify virtual address space memory isolation.

### 1.3 Tier 3: The Vault (`secure/`) & Hardware Passthrough
- **BitVisor 3.0 VT-d / DMAR Virtualization**:
  - Parses ACPI DMAR tables (`AcpiDrhdEntry`) to identify physical IOMMU remapping units.
  - Implements 4-level translation walks protecting host physical memory from guest PCIe bus mastering.
  - Permissions: `None` (`0b00`), `ReadOnly` (`0b01`), `WriteOnly` (`0b10`), `ReadWrite` (`0b11`).
- **NVIDIA OpenShell VFIO Hardware Passthrough**:
  - Validates BDF notation (`0000:01:00.0`), identifies 3D and VGA graphics devices (`0x030000` / `0x030200`).
  - Implements RAII `PciBindGuard` to safely unbind devices from host drivers (`nvidia`/`nouveau`), bind to `vfio-pci`, and automatically rebind on exit.
- **OpenSandbox 1.2.0 Zero-Trust Isolation**:
  - Mandates non-root UID/GID workload identity (rejects UID 0).
  - Binds runtimes to cryptographic SHA-256 tokens (`ProvisioningEvidence`).
  - Enforces fail-closed network mediation restricted to loopback (`127.0.0.0/8`).
- **PairDrop & LocalSend Integration**:
  - 3D Paper Airplane Courier coordinates WebRTC/WebSocket signaling and LocalSend REST v2 payloads with SHA-256 integrity verification.

---

## 2. VirtIO Block Device Driver Roadmap

To deliver native, high-throughput storage access in virtualized environments, Post Card OS integrates an OASIS VirtIO Block Device (v1.1) driver into `kernel/src/storage.rs` and `kernel/src/pci.rs`.

```
+-----------------------------------------------------------------------+
|                        Post Card Kernel Storage                       |
|           (StorageAccess / MBR Parser / FIFO Spooler)                 |
+-----------------------------------------------------------------------+
                                   |
                                   v
+-----------------------------------------------------------------------+
|                    VirtIO Block Driver (virtio_blk)                   |
|  - PCI Device Discovery (0x1AF4:0x1001 / 0x1AF4:0x1042)               |
|  - Feature Negotiation (VIRTIO_BLK_F_SIZE_MAX, VIRTIO_F_VERSION_1)    |
|  - Status Machine (ACKNOWLEDGE -> DRIVER -> FEATURES_OK -> DRIVER_OK) |
+-----------------------------------------------------------------------+
                                   |
         +-------------------------+-------------------------+
         |                                                   |
         v                                                   v
+-------------------------------+   +-----------------------------------+
|      Virtqueue Split Ring     |   |          DMA Buffer Pool          |
|  - Descriptor Table (16 bytes)|   |  - Page-aligned contiguous frames |
|  - Available Ring (Driver->Dev)|  |  - BitVisor DMAR-protected bounds|
|  - Used Ring (Dev->Driver)    |   |  - Zero-copy sector payload I/O   |
+-------------------------------+   +-----------------------------------+
                                   |
                                   v
+-----------------------------------------------------------------------+
|               PCI Transport & Interrupts (Legacy / Modern)            |
|  - I/O Port / MMIO Notify Register (`outw` / `writel`)                |
|  - INTx IRQ Handler (PIC Vector 43 / MSI-X Vector 0)                  |
|  - Fallback Polling Mode for Synchronous Early Boot Storage           |
+-----------------------------------------------------------------------+
```

### 2.1 PCI Device Discovery
- **Vendor ID**: `0x1AF4` (Red Hat / VirtIO).
- **Device IDs**:
  - `0x1001`: Legacy VirtIO Block Device.
  - `0x1042`: Modern VirtIO Block Device (OASIS standard v1.0+).
- **Transport Registers**:
  - `0x00`: Device Features (32-bit R)
  - `0x04`: Guest Features (32-bit R/W)
  - `0x08`: Queue Address (32-bit R/W, Page Frame Number)
  - `0x0C`: Queue Size (16-bit R)
  - `0x0E`: Queue Select (16-bit R/W)
  - `0x10`: Queue Notify (16-bit R/W)
  - `0x12`: Device Status (8-bit R/W)
  - `0x13`: ISR Status (8-bit R)

### 2.2 Device Initialization Sequence
1. **Reset**: Write `0x00` to `Device Status`.
2. **Acknowledge**: Write `0x01` (`ACKNOWLEDGE`).
3. **Driver**: Write `0x03` (`ACKNOWLEDGE | DRIVER`).
4. **Feature Negotiation**:
   - Bit 5: `VIRTIO_BLK_F_RO` (Read-Only disk).
   - Bit 6: `VIRTIO_BLK_F_BLK_SIZE` (Sector size, default 512).
   - Bit 9: `VIRTIO_BLK_F_FLUSH` (Cache flush support).
   - Bit 32: `VIRTIO_F_VERSION_1` (Modern standard).
5. **Features OK**: Write `0x0B` (`ACKNOWLEDGE | DRIVER | FEATURES_OK`) and verify device accepted features.
6. **Virtqueue Setup**: Read Queue 0 size, allocate contiguous physical memory for Descriptor Table, Available Ring, and Used Ring.
7. **Driver OK**: Write `0x0F` (`ACKNOWLEDGE | DRIVER | FEATURES_OK | DRIVER_OK`).

### 2.3 3-Descriptor Chained Request Protocol
Every sector I/O request is framed using three chained descriptors:
1. **Descriptor 1 (Header, Read-Only)**: Contains `VirtioBlkReqHeader` (`req_type`: 0=IN, 1=OUT, 4=FLUSH; `sector`: 64-bit LBA). `flags = VIRTQ_DESC_F_NEXT`.
2. **Descriptor 2 (Data Buffer)**: Contiguous RAM payload (`count * 512` bytes). For READ: `flags = VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE`; for WRITE: `flags = VIRTQ_DESC_F_NEXT`.
3. **Descriptor 3 (Status Byte, Device-Writable)**: 1-byte RAM status (`0`=OK, `1`=IOERR, `2`=UNSUPP). `flags = VIRTQ_DESC_F_WRITE`.

---

## 3. Golden Image Loading Protocol

The system provides a standardized 6-stage lifecycle for acquiring, converting, configuring, and verifying golden OS images, with explicit interactive collaboration checkpoints.

```
+-------------------------------------------------------------------------+
|                  Stage 1: Image Ingestion & Discovery                   |
|  - Check archive/guest-vm-setup/ for raw ISO / VHD images               |
|  - [USER INTERACTION CHECKPOINT]: Confirm paths, licenses, custom ISOs |
+-------------------------------------------------------------------------+
                                   |
                                   v
+-------------------------------------------------------------------------+
|                Stage 2: Format Standardization & Optimization           |
|  - Convert ISO/VHD/RAW to sparse qcow2 with zstd compression            |
|  - Generate base backing template (golden-base.qcow2)                   |
|  - Create copy-on-write overlay (card-active.qcow2)                     |
+-------------------------------------------------------------------------+
                                   |
                                   v
+-------------------------------------------------------------------------+
|            Stage 3: Unattended Setup & VirtIO Driver Injection          |
|  - Windows: autounattend.xml + virtio-win-gt-x64.msi + GPU-PV scripts   |
|  - Linux: Cloud-init (user-data, meta-data) with ttyS0 serial console   |
|  - Kali: SecOps tools pre-bake + host-only TAP bridge                   |
|  - Post Card OS: Bare-metal raw MBR disk + kernel ELF boot              |
+-------------------------------------------------------------------------+
                                   |
                                   v
+-------------------------------------------------------------------------+
|                 Stage 4: VM Orchestration & Registration                |
|  - Register guest metadata in workspace-prototype/guest-api.mjs        |
|  - Allocate port slots (Websocket: 5700+idx, QMP: 4444+idx, COM1: 4544) |
|  - Allocate memory & vCPU profiles (eco=2, balanced=4, gaming=8)       |
+-------------------------------------------------------------------------+
                                   |
                                   v
+-------------------------------------------------------------------------+
|               Stage 5: Live Verification & Self-Test Boot               |
|  - QEMU headless boot verification with QMP status check                |
|  - Serial console output validation (COM1 banner detection)             |
|  - noVNC RFB display connection handshake                               |
+-------------------------------------------------------------------------+
                                   |
                                   v
+-------------------------------------------------------------------------+
|                 Stage 6: Golden Snapshot Commitment                     |
|  - Execute qemu-img snapshot -c golden-v1.0 card-active.qcow2           |
|  - Seal golden template as read-only base                               |
|  - [USER CONFIRMATION]: Golden image ready for instantaneous card clone |
+-------------------------------------------------------------------------+
```

### 3.1 Operating System Matrix & Specifications

| Target OS | Preferred Source Format | Injected Drivers / Config | Target Hardware Profile | User Assistance Role |
|---|---|---|---|---|
| **Post Card Spatial OS** | `target/postcard-os.raw` (512 MiB MBR) | Native Rust microkernel + ramdisk | 1 vCPU, 256 MiB RAM, COM1 serial console | Automated build; verify boot prompt `spatial> ` |
| **Windows 11 / 10** | Rufus unattended VHD / ISO | `virtio-win-gt-x64.msi`, `autounattend.xml`, `GPU-PV` scripts | 4 vCPU, 4096 MiB RAM, GTX 1650 GPU-PV | Provide Windows ISO/Product ID, assist with initial setup |
| **Ubuntu / Debian Linux** | Official cloud-init `.qcow2` | Cloud-init NoCloud seed, `qemu-guest-agent`, `serial=ttyS0` | 2 vCPU, 2048 MiB RAM, virtio-net, virtio-blk | Confirm preferred default user credentials |
| **Kali Linux (SecOps)** | Kali Live / Installer ISO | Isolated TAP VLAN, OpenShell network supervisor, Metasploit pre-baked | 4 vCPU, 4096 MiB RAM, host-only bridge | Confirm target cyber range network subnet |
| **macOS (Sonoma/Sequoia)**| KVM-OpenCore recovery image | `OpenCore.qcow2`, `OSX-KVM` runner | 4 vCPU, 4096 MiB RAM, eval/research only | Note Apple EULA bounds; supply recovery DMG if desired |
| **Android-x86 / BlissOS** | `android-x86_64.iso` | VirtIO Tablet input, Virgl 3D GPU acceleration | 2 vCPU, 2048 MiB RAM, Touch/Tablet mode | Confirm preferred Android edition (BlissOS vs Android-x86) |

### 3.2 User Real-Time Assistance Protocol
- The user has confirmed active real-time availability: *"tell them any questions im right here and i have to help load the golden images"*.
- For proprietary images (Windows 11, macOS), the system prompts the user with exact paths and requirements rather than attempting guessing or downloading unauthorized files.
- The user can assist with unattended Windows setup by observing the live noVNC display inside the 3D studio.
- When an image passes self-tests, the user confirms final sealing of the immutable golden template.

---

## 4. Commercial Monetization Matrix

The commercial tiers map open-core and enterprise capabilities directly across technical boundaries in Rust crates and frontend modules:

| Feature / Dimension | Community ($0) | Pro Gamer ($19/mo or $190/yr) | Enterprise SecOps ($199/mo/seat or $1,990/yr/seat) |
|---|---|---|---|
| **License Type** | Open-Core / FOSS | Proprietary Pro License | Enterprise Multi-Seat SLA |
| **Microkernel Core (`kernel/`)** | Ring 0 Multiboot2, GDT, IDT, Paging, Shell | Ring 0 Microkernel, Extended Heap Pool | Ring 0 Microkernel, Enterprise Kernel Hooks |
| **Userland ABI (`user/`)** | `hello`, `poke`, `peek`, `evil` | Custom Userland Compilation Pipeline | Enterprise Audit Binaries & Security Tools |
| **Vault & Security (`secure/`)** | Basic MBR storage & memory FIFO spooler | Storage Spooler + PairDrop Network Transfer | BitVisor VT-d DMAR, NVIDIA OpenShell VFIO, OpenSandbox |
| **Compute / vCPU Profile** | `eco` (2 vCPU, 1024 MiB RAM per VM) | `gaming` (4 vCPU, 4096 MiB RAM per VM) | `unrestricted` (up to 16 vCPUs & 16 GiB RAM) |
| **Studio Graphics & UI** | Standard 3D Carousel, 30 FPS Recording | 60 FPS WebM Recording, Soft Shadows, Motes | SecOps Cyber Range switch, Multi-Host Dashboards |
| **Disk Snapshots** | Single active QCOW2 layer | Time-Travel Savepoints (up to 5 per card) | Unlimited Branching Snapshots, Swarm x10 Cloning |
| **Hardware Passthrough** | Software emulated VGA / QEMU stdvga | GPU-PV virtual graphics acceleration | Dedicated PCIe VFIO GPU passthrough (GTX 1650) |
| **Stripe Metadata** | `tier: "community"`, price: $0 | `tier: "pro"`, price: $19/mo or $190/yr | `tier: "enterprise"`, price: $199/mo/seat |

---

## 5. Reviewer 2 Polish Verifications

All non-blocking issues identified in the Milestone 5 review have been remediated:
1. **MediaStreamTrack Resource Leak**: Module-scoped `activeCaptureStream` tracks are stopped in `mediaRecorder.onstop` and `stopTourRecording()`.
2. **MediaRecorder Guard & Error Handling**: Verified `typeof MediaRecorder === "undefined"` and assigned `mediaRecorder.onerror`.
3. **Card Telemetry Facet Host**: Hardcoded `127.0.0.1` replaced with dynamic `host = window.location.hostname || "127.0.0.1"`.
4. **Button Focus Navigation**: Added `activeTag === "BUTTON"` and `pricingDialogElement.open` to keydown interception bypass.
5. **Tour Interruption Hotkeys**: `onCanvasKeyDown` ignores navigation keys while `tourActive` is true.
6. **Test Discovery**: `run-vite.mjs` dynamically runs all `tests/*.test.mjs` (21+ tests passing).
