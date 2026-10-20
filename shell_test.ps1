# Boot the ISO headless, type into the shell through the QEMU monitor (PS/2
# scancodes) and dump the VGA text screen.
# Usage: .\shell_test.ps1 -Keys "e c h o spc h i ret up" [-BootSeconds 9]
# (Run boot_test.ps1 first to build the ISO.)
param(
    [string]$Keys = "e c h o spc h i ret",
    [int]$BootSeconds = 9,
    [string]$Qemu = "C:\Program Files\qemu\qemu-system-x86_64.exe"
)
Set-Location $PSScriptRoot
$args = @("-cdrom", "target\postcard-spatial-os.iso", "-boot", "d",
          "-serial", "file:target\serial2.log", "-display", "none", "-no-reboot",
          "-monitor", "tcp:127.0.0.1:5577,server,nowait")
$p = Start-Process -FilePath $Qemu -ArgumentList $args -PassThru
Start-Sleep $BootSeconds
$c = New-Object System.Net.Sockets.TcpClient("127.0.0.1", 5577)
$s = $c.GetStream()
$w = New-Object System.IO.StreamWriter($s); $w.AutoFlush = $true
$r = New-Object System.IO.StreamReader($s)
function Send($cmd) { $w.WriteLine($cmd); Start-Sleep -Milliseconds 150 }
foreach ($k in $Keys.Split(" ")) { Send "sendkey $k" }
Start-Sleep -Milliseconds 500
Send "xp /2000xh 0xb8000"
Start-Sleep -Milliseconds 800
$buf = ""
while ($s.DataAvailable) { $buf += [char]$s.ReadByte() }
$p.Kill()
# Decode "0xADDR: 0x0f41 0x0f42 ..." lines: low byte of each halfword is the char.
$chars = New-Object System.Collections.Generic.List[char]
foreach ($line in $buf -split "`n") {
    if ($line -match "^\s*0*([0-9a-f]+):\s+(.*)$" -or $line -match "^.*?(0x[0-9a-f]+):\s+(.*)$") {
        foreach ($m in [regex]::Matches($line.Substring($line.IndexOf(":") + 1), "0x([0-9a-f]{4})")) {
            $v = [Convert]::ToInt32($m.Groups[1].Value, 16)
            $ch = $v -band 0xFF
            $chars.Add($(if ($ch -ge 32 -and $ch -lt 127) { [char]$ch } else { ' ' }))
        }
    }
}
$text = -join $chars
for ($i = 0; $i -lt [Math]::Min(25, [Math]::Floor($text.Length / 80)); $i++) {
    $row = $text.Substring($i * 80, 80).TrimEnd()
    if ($row.Length) { Write-Host $row }
}
