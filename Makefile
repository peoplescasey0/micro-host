# Canonical Linux Makefile for Post Card Spatial OS (micro-host)
# Master Command Center coordinating: kernel/, user/, secure/, scripts/
SHELL := /usr/bin/env bash
.SHELLFLAGS := -euo pipefail -c

CARGO ?= cargo
TARGET ?= x86_64-unknown-none
CARGO_INCREMENTAL ?= 0
export CARGO_INCREMENTAL

# Ensure cargo and local node/npm are in PATH
export PATH := $(HOME)/.local/bin:$(HOME)/.cargo/bin:$(PATH)

KERNEL_BIN := target/$(TARGET)/release/postcard-minimal-kernel
ISO_FILE := target/postcard-spatial-os.iso
RAW_DISK := target/postcard-os.raw
RAMDISK_IMG := ramdisk.img

.PHONY: all check userland ramdisk secure kernel iso disk qemu-boot qemu-shell test test-e2e clean help frontend stripe-probe stripe-sync

all: userland ramdisk secure kernel iso

check: ramdisk
	@echo "=== Checking userland crate (user/) ==="
	cd user && $(CARGO) check --target $(TARGET)
	@echo "=== Checking secure vault crate (secure/) ==="
	cd secure && $(CARGO) check
	@echo "=== Checking bare-metal kernel crate (kernel/) ==="
	cd kernel && $(CARGO) check --target $(TARGET)
	@echo "All cargo checks passed with 0 errors."

userland:
	@echo "=== Building userland binaries and packing ramdisk ==="
	./scripts/build_user.sh

ramdisk: userland

secure:
	@echo "=== Building secure host vault crate (secure/) ==="
	cd secure && $(CARGO) build

kernel: ramdisk
	@echo "=== Building bare-metal kernel (kernel/) ==="
	./scripts/build_kernel.sh

iso: kernel
	@echo "=== Creating bootable Multiboot2 ISO ==="
	./scripts/create_iso.sh

disk: kernel
	@echo "=== Installing Post Card OS to raw disk image ==="
	./scripts/install_postcard_os_wsl.sh $(RAW_DISK) $(KERNEL_BIN)

frontend:
	@echo "=== Building workspace-prototype frontend ==="
	cd workspace-prototype && npm run build

stripe-probe:
	@echo "=== Probing Stripe API Key Capabilities & Catalog ==="
	node scripts/stripe_sync.mjs --probe

stripe-sync:
	@echo "=== Synchronizing Stripe Monetization Catalog ==="
	node scripts/stripe_sync.mjs --sync --update-env

qemu-boot: iso
	@echo "=== Booting Post Card OS in QEMU (Headless) ==="
	./scripts/run_postcard_os.sh --headless

qemu-shell: iso
	@echo "=== Running QEMU Interactive Serial Shell Test ==="
	./scripts/shell_test.sh

test: iso disk
	@echo "=== Running Secure Vault Unit Tests ==="
	cd secure && $(CARGO) test
	@echo "=== Running Automated ISO Boot Test ==="
	./scripts/boot_test.sh
	@echo "=== Running Automated Raw Disk Boot Test ==="
	./scripts/boot_test.sh --disk
	@echo "=== Running Automated ISO Shell Test ==="
	./scripts/shell_test.sh
	@echo "=== Running Automated Raw Disk Shell Test ==="
	./scripts/shell_test.sh --disk
	@echo "All tests passed successfully!"

test-e2e:
	@echo "=== Running Dual-Track E2E Test Suite (Tiers 1-4) ==="
	./tests/run_e2e_tests.sh

clean:
	@echo "=== Cleaning build artifacts ==="
	rm -rf target/
	cd user && $(CARGO) clean
	cd secure && $(CARGO) clean
	cd kernel && $(CARGO) clean
	rm -rf $(RAMDISK_IMG)
	@echo "Clean completed."

help:
	@echo "Post Card Spatial OS Build Targets:"
	@echo "  make all          - Build userland, ramdisk, secure vault, kernel, and ISO"
	@echo "  make check        - Run cargo check on user, secure, and kernel crates"
	@echo "  make userland     - Build user programs and pack ramdisk.img"
	@echo "  make ramdisk      - Alias for userland"
	@echo "  make secure       - Build secure host vault crate"
	@echo "  make kernel       - Compile release bare-metal kernel ELF via scripts/build_kernel.sh"
	@echo "  make iso          - Create bootable Multiboot2 ISO via scripts/create_iso.sh"
	@echo "  make disk         - Prepare raw virtual disk (target/postcard-os.raw)"
	@echo "  make frontend     - Build workspace-prototype frontend (Vite & TypeScript)"
	@echo "  make stripe-probe - Probe Stripe API key capabilities and catalog status"
	@echo "  make stripe-sync  - Idempotently sync Stripe products/prices and update .env"
	@echo "  make qemu-boot    - Run QEMU with serial console"
	@echo "  make qemu-shell   - Run automated interactive serial shell test"
	@echo "  make test         - Run secure vault unit tests, boot test, and shell test"
	@echo "  make test-e2e     - Run Comprehensive Dual-Track E2E Test Suite (Tiers 1-4)"
	@echo "  make clean        - Remove target build artifacts"
