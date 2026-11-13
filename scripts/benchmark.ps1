param(
    [string]$Executable = (Join-Path $PSScriptRoot '..\dist\Undertow.exe'),
    [string]$AudioFile,
    [int]$Seconds = 180,
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '..\benchmarks'),
    [switch]$FloatingLyrics,
    [switch]$Background,
    [switch]$Mute,
    [switch]$Gpu,
    [switch]$AutoplayFavorites,
    [switch]$RequireTidal,
    [switch]$LibraryView,
    [switch]$LyricsView,
    [string]$Playlist,
    [string]$Album,
    [string]$Artist,
    [string]$SearchQuery,
    [switch]$SearchPreview,
    [switch]$CaptureSeries,
    [switch]$ArtworkScroll,
    [string]$WindowSize,
    [int]$Fps=120,
    [switch]$AutoplayAlbum,
    [switch]$ShowSidebar,
    [switch]$Collapsed
)
$ErrorActionPreference = 'Stop'
$executablePath = (Resolve-Path -LiteralPath $Executable).Path
if ($Seconds -lt 15 -or $Seconds -gt 7200) { throw 'Seconds must be between 15 and 7200.' }
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$outputPath = (Resolve-Path -LiteralPath $OutputDirectory).Path
$reportPath = Join-Path $outputPath 'application.json'
$screenshotPath = Join-Path $outputPath 'playback.png'
$arguments = [System.Collections.Generic.List[string]]::new()
$arguments.AddRange([string[]]@('--benchmark', [string]$Seconds, '--report', ('"' + $reportPath + '"'), '--screenshot', ('"' + $screenshotPath + '"')))
if ($Playlist) { $arguments.Add('--playlist'); $arguments.Add($Playlist) }
if ($Album) { $arguments.Add('--album'); $arguments.Add($Album) }
if ($Artist) { $arguments.Add('--artist'); $arguments.Add($Artist) }
if ($SearchQuery) { $arguments.Add($(if($SearchPreview){'--search-preview'}else{'--search'})); $arguments.Add($SearchQuery) }
if ($ArtworkScroll) { $arguments.Add('--artwork-scroll-benchmark') }
if ($CaptureSeries) { $arguments.Add('--screenshot-series') }
if ($WindowSize) { $arguments.Add('--window-size'); $arguments.Add($WindowSize) }
if ($LyricsView -or (-not $LibraryView -and -not $Playlist -and -not $Album -and -not $SearchQuery)) { $arguments.Add('--lyrics') }
if ($AudioFile) {
    $audioPath = (Resolve-Path -LiteralPath $AudioFile).Path
    $arguments.Add('--open'); $arguments.Add('"' + $audioPath + '"')
}
$arguments.Add('--fps'); $arguments.Add([string]$Fps)
if ($AutoplayAlbum) { $arguments.Add('--autoplay-album') }
if ($ShowSidebar) { $arguments.Add('--show-sidebar') }
if ($Collapsed) { $arguments.Add('--collapsed') }
if ($Background) {
    $arguments.Add('--background-benchmark')
    if (-not $Mute) { throw 'Background benchmarks must use -Mute.' }
}
if ($FloatingLyrics) { $arguments.Add('--floating') }
if ($Mute) { $arguments.Add('--mute') }
if ($Gpu) { $arguments.Add('--gpu') }
if ($AutoplayFavorites) { $arguments.Add('--autoplay-favorites') }
# Visible tests use the normal instance. Background tests create only invisible, non-activating windows and can coexist with the normal app.
$playerProcess = Start-Process -FilePath $executablePath -ArgumentList $arguments.ToArray() -PassThru -WindowStyle $(if ($Background) {'Hidden'} else {'Normal'})
if ($Background) { try { $playerProcess.PriorityClass = 'BelowNormal' } catch { } }
$samples = [System.Collections.Generic.List[object]]::new()
$timer = [System.Diagnostics.Stopwatch]::StartNew()
while (-not $playerProcess.HasExited -and $timer.Elapsed.TotalSeconds -lt ($Seconds + 30)) {
    $tree = @(Get-CimInstance Win32_Process | Where-Object { $_.ProcessId -eq $playerProcess.Id -or $_.ParentProcessId -eq $playerProcess.Id })
    $nativeProcesses = @($tree | Where-Object { $_.ExecutablePath -eq $executablePath })
    [long]$workingSet = 0; [long]$privateBytes = 0; [double]$cpuSeconds = 0
    foreach ($entry in $nativeProcesses) {
        $nativeProcess = Get-Process -Id $entry.ProcessId -ErrorAction SilentlyContinue
        if ($nativeProcess) { $workingSet += $nativeProcess.WorkingSet64; $privateBytes += $nativeProcess.PrivateMemorySize64; $cpuSeconds += $nativeProcess.TotalProcessorTime.TotalSeconds }
    }
    $samples.Add([pscustomobject]@{seconds=$timer.Elapsed.TotalSeconds;processes=$nativeProcesses.Count;working_set_bytes=$workingSet;private_bytes=$privateBytes;cpu_seconds=$cpuSeconds})
    Start-Sleep -Milliseconds 1000
    $playerProcess.Refresh()
}
if (-not $playerProcess.HasExited) { throw 'Player did not finish the benchmark within its deadline; inspect the still-running application.' }
if (-not (Test-Path -LiteralPath $reportPath)) { throw 'No application report was produced. Another instance may have been running.' }
$application = Get-Content -LiteralPath $reportPath -Raw | ConvertFrom-Json
$peakWorkingSet = ($samples | Measure-Object working_set_bytes -Maximum).Maximum
$peakPrivate = ($samples | Measure-Object private_bytes -Maximum).Maximum
$consumedSeconds = if ($null -ne $application.details.consumed_audio_seconds) { $application.details.consumed_audio_seconds } else { $application.details.played_seconds }
$playbackVerified = $application.details.playback_active -and $application.details.codec -eq 'FLAC' -and $consumedSeconds -ge ($Seconds * 0.8)
$tidalVerified = $application.details.authenticated -and $application.details.tidal_stream
$memoryBudget = [long]$application.budget_bytes
$softTarget = 200000000
$memoryPassed = $peakWorkingSet -gt 0 -and $peakWorkingSet -lt $memoryBudget -and $peakPrivate -lt $memoryBudget -and $application.within_process_memory_budget
$report = [pscustomobject]@{
    background_benchmark=[bool]$Background
    scope='Release native process and optional native lyrics child; shared working-set pages conservatively counted twice. External Discord/browser/Windows services are not app processes. GPU memory is separate.'
    budget_bytes=$memoryBudget
    requested_target_bytes=$application.requested_target_bytes
    within_requested_target=($application.within_requested_target -and $peakWorkingSet -lt $application.requested_target_bytes -and $peakPrivate -lt $application.requested_target_bytes)
    soft_target_bytes=$softTarget
    within_original_target=($peakWorkingSet -gt 0 -and $peakWorkingSet -lt $softTarget -and $peakPrivate -lt $softTarget)
    executable_sha256=(Get-FileHash -LiteralPath $executablePath -Algorithm SHA256).Hash.ToLowerInvariant()
    consumed_audio_seconds=$consumedSeconds
    peak_working_set_bytes=$peakWorkingSet
    peak_private_bytes=$peakPrivate
    memory_passed=$memoryPassed
    actual_flac_playback_verified=$playbackVerified
    tidal_playback_verified=$tidalVerified
    requested_full_scenario_verified=$false
    note='All sixteen live feature checks, playlist write round trips, and external provider availability require separate evidence. Enabled settings alone are not validation.'
    samples=$samples
}
$report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $outputPath 'process-tree.json') -Encoding utf8
$report | Select-Object peak_working_set_bytes,peak_private_bytes,memory_passed,actual_flac_playback_verified,tidal_playback_verified
if (-not $memoryPassed -or -not $playbackVerified -or ($RequireTidal -and -not $tidalVerified)) { exit 1 }
