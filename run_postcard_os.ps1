param(
    [string]$ImagePath = (Join-Path $PSScriptRoot "target\postcard-os.raw"),
    [string]$QemuExe = "C:\Program Files\qemu\qemu-system-x86_64.exe"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $ImagePath)) {
    throw "Post Card OS disk image not found: $ImagePath. Run .\install_postcard_os.ps1 first."
}
if (-not (Test-Path $QemuExe)) {
    throw "QEMU executable not found: $QemuExe"
}

$arguments = @(
    "-name", "Post Card OS",
    "-machine", "pc",
    "-m", "256M",
    "-drive", "file=$ImagePath,format=raw,if=ide,index=0",
    "-boot", "order=c",
    "-display", "sdl",
    "-serial", "file:target\postcard-os-serial.log",
    "-no-reboot"
)

& $QemuExe @arguments
if ($LASTEXITCODE -ne 0) {
    throw "QEMU exited with code $LASTEXITCODE"
}
