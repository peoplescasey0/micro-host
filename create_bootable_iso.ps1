# Build a bootable Multiboot2 ISO for Post Card OS.

$ErrorActionPreference = "Stop"
Set-Location -Path $PSScriptRoot

$kernel = Join-Path $PSScriptRoot "target\x86_64-unknown-none\release\postcard-minimal-kernel"
$isoDir = Join-Path $PSScriptRoot "target\iso"
$bootDir = Join-Path $isoDir "boot"
$grubDir = Join-Path $bootDir "grub"
$isoFile = Join-Path $PSScriptRoot "target\postcard-spatial-os.iso"

Write-Host "Building kernel..."
& cargo build --release --target x86_64-unknown-none
if ($LASTEXITCODE -ne 0) {
    throw "Kernel build failed with exit code $LASTEXITCODE"
}

if (-not (Test-Path $kernel)) {
    throw "Kernel ELF not found: $kernel"
}

New-Item -ItemType Directory -Force -Path $grubDir | Out-Null
Copy-Item -Path $kernel -Destination (Join-Path $bootDir "kernel.elf") -Force

$grubConfig = Join-Path $grubDir "grub.cfg"
Set-Content -Path $grubConfig -Value "set timeout=0" -Encoding ASCII
Add-Content -Path $grubConfig -Value "set default=0" -Encoding ASCII
Add-Content -Path $grubConfig -Value 'menuentry "Post Card Spatial OS" {' -Encoding ASCII
Add-Content -Path $grubConfig -Value "    multiboot2 /boot/kernel.elf" -Encoding ASCII
Add-Content -Path $grubConfig -Value "    boot" -Encoding ASCII
Add-Content -Path $grubConfig -Value "}" -Encoding ASCII

if (Get-Command "grub-mkrescue" -ErrorAction SilentlyContinue) {
    & grub-mkrescue -o $isoFile $isoDir
} elseif (Get-Command "wsl.exe" -ErrorAction SilentlyContinue) {
    $linuxIsoFile = (& wsl.exe -e wslpath -a $isoFile).Trim()
    $linuxIsoDir = (& wsl.exe -e wslpath -a $isoDir).Trim()
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to convert ISO paths for WSL"
    }
    & wsl.exe -e grub-mkrescue -o $linuxIsoFile $linuxIsoDir
} else {
    throw "grub-mkrescue is required on Windows or in WSL"
}

if ($LASTEXITCODE -ne 0) {
    throw "grub-mkrescue failed with exit code $LASTEXITCODE"
}
if (-not (Test-Path $isoFile)) {
    throw "ISO was not created: $isoFile"
}

Write-Host "Bootable ISO created: $isoFile"