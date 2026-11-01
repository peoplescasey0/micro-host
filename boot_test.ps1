# Build ISO, boot headless in QEMU for N seconds, capture serial to target\serial.log.
# Usage: .\boot_test.ps1 [-Seconds 10] [-DebugLog]
param(
    [int]$Seconds = 10,
    [switch]$DebugLog,
    [string]$Qemu = "C:\Program Files\qemu\qemu-system-x86_64.exe"
)
Set-Location $PSScriptRoot
$ErrorActionPreference = "Continue"

& powershell -NoProfile -ExecutionPolicy Bypass -File .\create_bootable_iso.ps1 *> target\iso_build.log
if (-not (Test-Path target\postcard-spatial-os.iso)) { Write-Host "ISO build failed, see target\iso_build.log"; exit 1 }

Remove-Item target\serial.log, target\qemu.log -ErrorAction SilentlyContinue
$args = @("-cdrom", "target\postcard-spatial-os.iso", "-boot", "d",
          "-serial", "file:target\serial.log", "-display", "none", "-no-reboot")
if ($DebugLog) { $args += @("-d", "int,cpu_reset", "-D", "target\qemu.log") }

$p = Start-Process -FilePath $Qemu -ArgumentList $args -PassThru
Start-Sleep $Seconds
if (-not $p.HasExited) { $p.Kill(); Write-Host "QEMU still running after ${Seconds}s (killed)" }
else { Write-Host "QEMU exited early with code $($p.ExitCode) (triple fault / crash?)" }
Write-Host "---- serial ----"
if (Test-Path target\serial.log) { Get-Content target\serial.log }
