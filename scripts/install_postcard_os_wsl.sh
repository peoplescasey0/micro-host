#!/usr/bin/env bash
set -euo pipefail

disk_image=${1:?disk image path required}
kernel_elf=${2:?kernel ELF path required}

if [[ ! -f "$disk_image" ]]; then
    echo "Creating 512 MiB raw disk image at $disk_image..."
    mkdir -p "$(dirname "$disk_image")"
    qemu-img create -f raw "$disk_image" 512M
fi

if [[ ! -f "$kernel_elf" ]]; then
    echo "Error: Kernel ELF does not exist at $kernel_elf" >&2
    exit 1
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Check if running as root with loop device access
if [[ "$(id -u)" = "0" ]] && command -v losetup &>/dev/null; then
    echo "=== Installing Post Card OS to disk via loopback (root mode) ==="
    loop_device=
    mount_dir=

    cleanup() {
        if [[ -n "$mount_dir" ]] && mountpoint -q "$mount_dir"; then
            umount "$mount_dir"
        fi
        if [[ -n "$mount_dir" ]]; then
            rmdir "$mount_dir"
        fi
        if [[ -n "$loop_device" ]]; then
            losetup -d "$loop_device"
        fi
    }
    trap cleanup EXIT

    loop_device=$(losetup --find --show --partscan "$disk_image")
    printf 'label: dos\nunit: sectors\n\nstart=2048, type=83, bootable\n' | sfdisk "$loop_device"
    partx -u "$loop_device" || true
    udevadm settle || true

    partition="${loop_device}p1"
    for _ in {1..20}; do
        [[ -b "$partition" ]] && break
        sleep 0.1
    done
    if [[ ! -b "$partition" ]]; then
        echo "Partition device was not created for $loop_device." >&2
        exit 1
    fi

    mkfs.ext4 -F "$partition"
    mount_dir=$(mktemp -d)
    mount "$partition" "$mount_dir"
    mkdir -p "$mount_dir/boot/grub"
    install -m 0644 "$kernel_elf" "$mount_dir/boot/kernel.elf"
    if [[ -f "$script_dir/ramdisk.img" ]]; then
        install -m 0644 "$script_dir/ramdisk.img" "$mount_dir/boot/ramdisk.img"
    fi
    cat > "$mount_dir/boot/grub/grub.cfg" <<'GRUB_CONFIG'
serial --unit=0 --speed=115200
terminal_input serial console
terminal_output serial console
set timeout=0
set default=0

menuentry "Post Card OS" {
    multiboot2 /boot/kernel.elf console=ttyS0 console=tty0
    if [ -f /boot/ramdisk.img ]; then
        module2 /boot/ramdisk.img ramdisk
    fi
    boot
}
GRUB_CONFIG

    grub-install --target=i386-pc --boot-directory="$mount_dir/boot" --recheck "$loop_device"
    sync
    umount "$mount_dir"
    rmdir "$mount_dir"
    mount_dir=
    losetup -d "$loop_device"
    loop_device=
    trap - EXIT
    echo "GRUB and Post Card OS kernel installed successfully (root mode)."
else
    echo "=== Installing Post Card OS to disk (user-space direct mode) ==="
    # 1. Partition disk with MBR and active partition starting at sector 2048
    printf 'label: dos\nunit: sectors\n\nstart=2048, type=83, bootable\n' | sfdisk --force "$disk_image"

    # 2. Prepare staging directory for ext4 filesystem
    staging_dir=$(mktemp -d)
    mkdir -p "$staging_dir/boot/grub"
    install -m 0644 "$kernel_elf" "$staging_dir/boot/kernel.elf"
    if [[ -f "$script_dir/ramdisk.img" ]]; then
        install -m 0644 "$script_dir/ramdisk.img" "$staging_dir/boot/ramdisk.img"
    fi

    cat > "$staging_dir/boot/grub/grub.cfg" <<'GRUB_CONFIG'
serial --unit=0 --speed=115200
terminal_input serial console
terminal_output serial console
set timeout=0
set default=0

menuentry "Post Card OS" {
    multiboot2 /boot/kernel.elf console=ttyS0 console=tty0
    if [ -f /boot/ramdisk.img ]; then
        module2 /boot/ramdisk.img ramdisk
    fi
    boot
}
GRUB_CONFIG

    # 3. Format ext4 partition at offset 1048576 (sector 2048, 130816 4K blocks for 511M)
    mke2fs -F -t ext4 -b 4096 -E offset=1048576 -d "$staging_dir" "$disk_image" 130816
    rm -rf "$staging_dir"

    # 4. Generate core.img and embed GRUB bootloader into MBR and sectors 1..N
    tmp_core=$(mktemp)
    grub-mkimage -O i386-pc -p "(hd0,msdos1)/boot/grub" -o "$tmp_core" biosdisk part_msdos ext2 normal multiboot2

    python3 - "$tmp_core" "$disk_image" << 'PYEOF'
import os
import struct
import sys

core_img_path = sys.argv[1]
disk_img_path = sys.argv[2]
boot_img_path = "/usr/lib/grub/i386-pc/boot.img"

if not os.path.exists(boot_img_path):
    print(f"Error: GRUB boot.img not found at {boot_img_path}", file=sys.stderr)
    sys.exit(1)

core_size = os.path.getsize(core_img_path)
total_sectors = (core_size + 511) // 512
remaining_sectors = total_sectors - 1

# Patch diskboot.S blocklist length at offset 0x1fc in core.img
with open(core_img_path, "r+b") as f:
    f.seek(0x1fc)
    f.write(struct.pack("<H", remaining_sectors))

with open(boot_img_path, "rb") as bf:
    boot_code = bf.read(512)

with open(disk_img_path, "r+b") as df:
    # Read existing MBR partition table from sector 0 (bytes 446 to 512)
    df.seek(446)
    ptable = df.read(66)
    
    # Write boot.img bootstrap code (preserving partition table and signature)
    df.seek(0)
    df.write(boot_code[:446])
    df.seek(446)
    df.write(ptable)
    
    # Embed core.img starting at sector 1 (offset 512)
    df.seek(512)
    with open(core_img_path, "rb") as cf:
        df.write(cf.read())

PYEOF
    rm -f "$tmp_core"

    echo "GRUB and Post Card OS kernel installed successfully (user-space direct mode)."
fi
