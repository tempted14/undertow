param([switch]$IncludeBroker)
$ErrorActionPreference = 'Stop'
$sourceRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$previousFlags = $env:CARGO_ENCODED_RUSTFLAGS
$separator = [string][char]0x1f
$flags = [System.Collections.Generic.List[string]]::new()
if ($previousFlags) {
    $flags.AddRange([string[]]$previousFlags.Split([char]0x1f))
} elseif ($env:RUSTFLAGS) {
    # Match Cargo's whitespace splitting for ordinary RUSTFLAGS; encoded flags
    # preserve each remapping argument even when the source path contains spaces.
    $flags.AddRange([string[]]($env:RUSTFLAGS -split '\s+' | Where-Object { $_ }))
}
$userDirectory = [Environment]::GetFolderPath('UserProfile')
foreach ($mapping in @(@($userDirectory,'/build/home'),@($sourceRoot,'/build/undertow'))) {
    if (-not $mapping[0]) { continue }
    foreach ($prefix in @($mapping[0],$mapping[0].Replace('\','/'))) {
        $flags.Add('--remap-path-prefix=' + $prefix + '=' + $mapping[1])
    }
}
try {
    $env:CARGO_ENCODED_RUSTFLAGS = $flags -join $separator
    Push-Location $sourceRoot
    try {
        & cargo build --release --locked --bin undertow
        if ($LASTEXITCODE -ne 0) { throw 'Desktop release build failed' }
        if ($IncludeBroker) {
            & cargo build --release --locked --features auth-broker --bin undertow-auth-broker
            if ($LASTEXITCODE -ne 0) { throw 'Broker release build failed' }
        }
    } finally { Pop-Location }
} finally {
    if ($null -eq $previousFlags) { Remove-Item Env:CARGO_ENCODED_RUSTFLAGS -ErrorAction SilentlyContinue }
    else { $env:CARGO_ENCODED_RUSTFLAGS = $previousFlags }
}
