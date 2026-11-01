param(
    [string]$ImagePath = (Join-Path $PSScriptRoot "target\postcard-os.raw"),
    [string]$QemuImgExe = "C:\Program Files\qemu\qemu-img.exe"
)

$ErrorActionPreference = "Stop"

if (Test-Path $ImagePath) {
    throw "Refusing to overwrite existing disk image: $ImagePath"
}
if (-not (Test-Path $QemuImgExe)) {
    throw "qemu-img executable not found: $QemuImgExe"
}
if (-not (Get-Command "wsl.exe" -ErrorAction SilentlyContinue)) {
    throw "WSL is required to partition and install GRUB on the virtual disk."
}

$wslInstaller = Join-Path $PSScriptRoot "install_postcard_os_wsl.sh"
$kernelPath = Join-Path $PSScriptRoot "target\x86_64-unknown-none\release\postcard-minimal-kernel"
if (-not (Test-Path $wslInstaller)) {
    throw "Disk installer helper not found: $wslInstaller"
}

Write-Host "Building Post Card OS kernel..."
Push-Location $PSScriptRoot
try {
    $previousRustFlags = $env:RUSTFLAGS
    try {
        & .\build_user.ps1
        if ($LASTEXITCODE -ne 0) {
            throw "User program build failed with exit code $LASTEXITCODE"
        }
    } finally {
        if ($null -eq $previousRustFlags) {
            Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
        } else {
            $env:RUSTFLAGS = $previousRustFlags
        }
    }

    & cargo build --release --target x86_64-unknown-none
    if ($LASTEXITCODE -ne 0) {
        throw "Kernel build failed with exit code $LASTEXITCODE"
    }
} finally {
    Pop-Location
}

if (-not (Test-Path $kernelPath)) {
    throw "Kernel ELF not found: $kernelPath"
}

$imageDirectory = Split-Path -Parent $ImagePath
if ($imageDirectory) {
    New-Item -ItemType Directory -Force -Path $imageDirectory | Out-Null
}

Write-Host "Creating 512 MiB raw virtual disk..."
& $QemuImgExe create -f raw $ImagePath 512M
if ($LASTEXITCODE -ne 0) {
    throw "Virtual disk creation failed with exit code $LASTEXITCODE"
}

$linuxInstaller = (& wsl.exe -e wslpath -a $wslInstaller).Trim()
$linuxImage = (& wsl.exe -e wslpath -a $ImagePath).Trim()
$linuxKernel = (& wsl.exe -e wslpath -a $kernelPath).Trim()
if ($LASTEXITCODE -ne 0 -or -not $linuxInstaller -or -not $linuxImage -or -not $linuxKernel) {
    throw "Unable to convert installer paths for WSL."
}

Write-Host "Partitioning the image, installing GRUB, and copying the kernel..."
& wsl.exe -u root -- bash $linuxInstaller $linuxImage $linuxKernel
if ($LASTEXITCODE -ne 0) {
    throw "Disk installation failed with exit code $LASTEXITCODE. The image was retained for inspection: $ImagePath"
}

Write-Host "Post Card OS installed to: $ImagePath"
Write-Host "Boot it with: .\run_postcard_os.ps1"
