param(
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '..\..'),
    [string]$Executable = (Join-Path $PSScriptRoot '..\target\release\undertow.exe')
)
$ErrorActionPreference = 'Stop'
$sourceRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$outputRoot = (Resolve-Path -LiteralPath $OutputDirectory).Path
$releaseExe = $Executable
if (-not (Test-Path -LiteralPath $releaseExe)) { throw 'Build the release desktop executable first.' }
& python (Join-Path $PSScriptRoot 'check-release.py') $releaseExe
if ($LASTEXITCODE -ne 0) { throw 'Release privacy check failed; use scripts/build-release.ps1.' }
$outputExe = Join-Path $outputRoot 'Undertow.exe'
if (-not (Test-Path -LiteralPath $outputExe) -or (Get-FileHash -LiteralPath $releaseExe -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath $outputExe -Algorithm SHA256).Hash) {
    Copy-Item -LiteralPath $releaseExe -Destination $outputExe -Force
}

Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
function Write-Archive([string]$ArchivePath, [object[]]$Entries) {
    # CreateNew prevents overwriting any existing release archive.
    $stream = [System.IO.File]::Open($ArchivePath, [System.IO.FileMode]::CreateNew)
    try {
        $zip = [System.IO.Compression.ZipArchive]::new($stream, [System.IO.Compression.ZipArchiveMode]::Create, $true)
        try {
            foreach ($entry in $Entries) {
                [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $entry.Path, $entry.Name.Replace('\','/'), [System.IO.Compression.CompressionLevel]::Optimal) | Out-Null
            }
        } finally { $zip.Dispose() }
    } finally { $stream.Dispose() }
}
$documents = @('README.md','VALIDATION.md','PLUGIN-FEATURES.md','RESEARCH.md','AUTH-BROKER.md','THIRD-PARTY.md','THIRD-PARTY-LICENSES.txt','LICENSE','assets/tidal-ui/README.md')
$portableEntries = @([pscustomobject]@{Path=$outputExe;Name='Undertow/Undertow.exe'})
$documentEntries = foreach ($document in $documents) {
    $relative = if (Test-Path -LiteralPath (Join-Path $sourceRoot $document)) { $document } else { "docs/$document" }
    $path = Join-Path $sourceRoot $relative
    if (-not (Test-Path -LiteralPath $path)) { throw "Missing distribution document: $document" }
    [pscustomobject]@{Path=$path;Relative=$relative}
}
foreach ($document in $documentEntries) { $portableEntries += [pscustomobject]@{Path=$document.Path;Name=("Undertow/"+$document.Relative)} }
if (Test-Path -LiteralPath (Join-Path $sourceRoot 'docs')) {
    $known = @($portableEntries | ForEach-Object { $_.Name })
    foreach ($file in Get-ChildItem -LiteralPath (Join-Path $sourceRoot 'docs') -File -Recurse) {
        $name = 'Undertow/' + [System.IO.Path]::GetRelativePath($sourceRoot,$file.FullName).Replace('\','/')
        if ($known -notcontains $name) { $portableEntries += [pscustomobject]@{Path=$file.FullName;Name=$name} }
    }
}
foreach ($file in Get-ChildItem -LiteralPath (Join-Path $sourceRoot 'licenses') -File -Recurse) {
    $portableEntries += [pscustomobject]@{Path=$file.FullName;Name=('Undertow/' + [System.IO.Path]::GetRelativePath($sourceRoot,$file.FullName))}
}
Write-Archive (Join-Path $outputRoot 'Undertow-Windows-x64.zip') $portableEntries

$sourceEntries = @()
foreach ($document in $documentEntries) { $sourceEntries += [pscustomobject]@{Path=$document.Path;Name=("undertow/"+$document.Relative)} }
foreach ($fileName in @('Cargo.toml','Cargo.lock','build.rs','.gitignore','.gitattributes')) {
    if (-not (Test-Path -LiteralPath (Join-Path $sourceRoot $fileName))) { continue }
    $sourceEntries += [pscustomobject]@{Path=(Join-Path $sourceRoot $fileName);Name="undertow/$fileName"}
}
foreach ($face in @('regular','medium','demibold')) {
    $relative = "assets/tidal-ui/nationale-$face.ttf"
    $path = Join-Path $sourceRoot $relative
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing UI typeface: $relative" }
    $sourceEntries += [pscustomobject]@{Path=$path;Name="undertow/$relative"}
}
foreach ($relative in @('assets/undertow.ico','assets/undertow.png','assets/undertow.rc')) {
    $path = Join-Path $sourceRoot $relative
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing application icon asset: $relative" }
    $sourceEntries += [pscustomobject]@{Path=$path;Name="undertow/$relative"}
}
# Explicit allowlist: never include target/, account data, caches, work files, or credentials.
foreach ($directory in @('src','vendor','tests','scripts','licenses','benchmarks','.github')) {
    if (-not (Test-Path -LiteralPath (Join-Path $sourceRoot $directory))) { continue }
    foreach ($file in Get-ChildItem -LiteralPath (Join-Path $sourceRoot $directory) -File -Recurse -Force) {
        if ($file.FullName -match '[\\/](__pycache__|target|\.git|artifacts)[\\/]') { continue }
        $sourceEntries += [pscustomobject]@{Path=$file.FullName;Name=('undertow/' + [System.IO.Path]::GetRelativePath($sourceRoot,$file.FullName))}
    }
}
if (Test-Path -LiteralPath (Join-Path $sourceRoot 'docs')) {
    $known = @($sourceEntries | ForEach-Object { $_.Name })
    foreach ($file in Get-ChildItem -LiteralPath (Join-Path $sourceRoot 'docs') -File -Recurse) {
        $name = 'undertow/' + [System.IO.Path]::GetRelativePath($sourceRoot,$file.FullName).Replace('\','/')
        if ($known -notcontains $name) { $sourceEntries += [pscustomobject]@{Path=$file.FullName;Name=$name} }
    }
}
Write-Archive (Join-Path $outputRoot 'Undertow-source.zip') $sourceEntries
$hashes = foreach ($fileName in @('Undertow.exe','Undertow-Windows-x64.zip','Undertow-source.zip')) {
    $hash = Get-FileHash -LiteralPath (Join-Path $outputRoot $fileName) -Algorithm SHA256
    "$($hash.Hash.ToLowerInvariant())  $fileName"
}
$hashes | Set-Content -LiteralPath (Join-Path $outputRoot 'SHA256SUMS.txt') -Encoding ascii
Get-Item -LiteralPath $outputExe,(Join-Path $outputRoot 'Undertow-Windows-x64.zip'),(Join-Path $outputRoot 'Undertow-source.zip') | Select-Object Name,Length
