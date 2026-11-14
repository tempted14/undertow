param([int]$TaskProcessId,[string]$OutputPath)
$ErrorActionPreference='Stop'
(Get-Process -Id $PID).PriorityClass='BelowNormal'
$rows=@()
Get-Counter -Counter '\GPU Engine(*)\Utilization Percentage','\GPU Process Memory(*)\Dedicated Usage','\GPU Process Memory(*)\Shared Usage' -SampleInterval 1 -MaxSamples 14 | ForEach-Object {
    $samples=@($_.CounterSamples | Where-Object { $_.InstanceName -like "pid_${TaskProcessId}_*" })
    $engine=($samples | Where-Object { $_.Path -like '*\utilization percentage' } | Measure-Object CookedValue -Sum).Sum
    $dedicated=($samples | Where-Object { $_.Path -like '*\dedicated usage' } | Measure-Object CookedValue -Sum).Sum
    $shared=($samples | Where-Object { $_.Path -like '*\shared usage' } | Measure-Object CookedValue -Sum).Sum
    $rows+=@{engine_percent_sum=$engine;dedicated_bytes=$dedicated;shared_bytes=$shared;matched_instances=$samples.Count}
}
ConvertTo-Json -Depth 4 -InputObject @($rows) | Set-Content -LiteralPath $OutputPath -Encoding utf8
