#!/usr/bin/env bash
set -euo pipefail

disk_image=${1:?disk image path required}
kernel_elf=${2:?kernel ELF path required}

if [[ ! -f "$disk_image" || ! -f "$kernel_elf" ]]; then
    echo "Disk image or kernel ELF does not exist." >&2
    exit 1
fi

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
partx -u "$loop_device"
udevadm settle

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
cat > "$mount_dir/boot/grub/grub.cfg" <<'GRUB_CONFIG'
set timeout=0
set default=0

menuentry "Post Card OS" {
    multiboot2 /boot/kernel.elf
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

echo "GRUB and Post Card OS kernel installed successfully."
