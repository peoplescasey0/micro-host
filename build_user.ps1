# Builds the user programs (user/) and packs them into ramdisk.img.
# Image format (little endian): "RDSK", u32 count, count * { name[24] NUL padded, u32 offset, u32 size }, data.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$previousRustFlags = $env:RUSTFLAGS
try {
    $env:RUSTFLAGS = "-C link-arg=-T$PSScriptRoot\user\user.ld -C relocation-model=static -C code-model=large"
    Push-Location user
    try {
        & cargo build --release
        if ($LASTEXITCODE -ne 0) { throw "user build failed with exit code $LASTEXITCODE" }
    } finally {
        Pop-Location
    }
} finally {
    if ($null -eq $previousRustFlags) {
        Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
    } else {
        $env:RUSTFLAGS = $previousRustFlags
    }
}

$dir = "user\target\x86_64-unknown-none\release"
$names = "hello", "evil", "poke", "peek"
$files = $names | ForEach-Object { , [System.IO.File]::ReadAllBytes("$PSScriptRoot\$dir\$_") }

$ms = New-Object System.IO.MemoryStream
$bw = New-Object System.IO.BinaryWriter($ms)
$bw.Write([System.Text.Encoding]::ASCII.GetBytes("RDSK"))
$bw.Write([uint32]$names.Count)
$off = 8 + 32 * $names.Count
for ($i = 0; $i -lt $names.Count; $i++) {
    $nb = New-Object byte[] 24
    [System.Text.Encoding]::ASCII.GetBytes($names[$i]).CopyTo($nb, 0)
    $bw.Write($nb)
    $bw.Write([uint32]$off)
    $bw.Write([uint32]$files[$i].Length)
    $off += $files[$i].Length
}
foreach ($f in $files) { $bw.Write($f) }
$bw.Flush()
[System.IO.File]::WriteAllBytes("$PSScriptRoot\ramdisk.img", $ms.ToArray())
Write-Host "ramdisk.img: $($ms.Length) bytes, $($names.Count) programs"
