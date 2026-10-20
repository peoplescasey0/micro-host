# Post Card OS - QEMU Run Script for Windows
# Runs the built kernel image in QEMU for testing and debugging

param(
    [switch]$Debug = $false,
    [switch]$NoGraphics = $true,
    [string]$QemuPath = "C:\Program Files\qemu\qemu-system-x86_64.exe"
)

Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "Post Card OS - Kernel Runner" -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan

# Check if QEMU exists
if (-not (Test-Path $QemuPath)) {
    Write-Host "QEMU not found at: $QemuPath" -ForegroundColor Red
    Write-Host "Please install QEMU from: https://www.qemu.org/download/#windows" -ForegroundColor Yellow
    Write-Host "Or update the QemuPath parameter in this script." -ForegroundColor Yellow
    exit 1
}

$isoPath = Join-Path $PSScriptRoot "target\postcard-spatial-os.iso"
Write-Host "Building the latest kernel ISO..." -ForegroundColor Yellow
& (Join-Path $PSScriptRoot "create_bootable_iso.ps1")
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $isoPath)) {
    Write-Host "ISO build failed. Run create_bootable_iso.ps1 to see details." -ForegroundColor Red
    exit 1
}

# Build QEMU command arguments
$qemuArgs = @(
    "-cdrom", $isoPath,
    "-boot", "d",
    "-serial", "stdio"
)

if ($NoGraphics) {
    $qemuArgs += "-display", "none"
}

if ($Debug) {
    Write-Host "Debug mode enabled - QEMU will wait for GDB connection on port 1234" -ForegroundColor Yellow
    $qemuArgs += @(
        "-s",  # Wait for GDB connection
        "-S",  # Freeze CPU at startup
        "-d", "int"  # Log interrupts
    )
} else {
    $qemuArgs += "-no-reboot"
}

# Add additional debugging options for kernel development
$qemuArgs += @(
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"
)

Write-Host "`nStarting QEMU with Post Card OS..." -ForegroundColor Green
Write-Host "QEMU Path: $QemuPath" -ForegroundColor Gray
Write-Host "Bootable ISO: $isoPath" -ForegroundColor Gray
Write-Host "Arguments: $($qemuArgs -join ' ')" -ForegroundColor Gray
Write-Host "`nPress Ctrl+C to stop QEMU" -ForegroundColor Yellow

if ($Debug) {
    Write-Host "`nDebug Info:" -ForegroundColor Cyan
    Write-Host "- QEMU is waiting for GDB connection on localhost:1234" -ForegroundColor Gray
    Write-Host "- Connect with: gdb -ex 'target remote localhost:1234'" -ForegroundColor Gray
    Write-Host "- Set breakpoints with: break *0x1000 (or your kernel entry point)" -ForegroundColor Gray
}

try {
    # Run QEMU
    & $QemuPath $qemuArgs
    
    if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne 1) {
        Write-Host "`nQEMU exited with code: $LASTEXITCODE" -ForegroundColor Red
    } else {
        Write-Host "`nQEMU stopped successfully" -ForegroundColor Green
    }
} catch {
    Write-Host "`nFailed to run QEMU: $_" -ForegroundColor Red
    exit 1
}

Write-Host "`n=========================================" -ForegroundColor Cyan
Write-Host "Post Card OS Execution Complete" -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan