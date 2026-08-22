[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $Binary,
    [int] $SettleSeconds = 2,
    [int] $IdleSeconds = 10,
    [switch] $Enforce,
    [double] $EventToVisibleMs
)

$ErrorActionPreference = 'Stop'
$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$budgetPath = Join-Path $scriptRoot 'performance-budgets.json'
$budgets = Get-Content -Raw $budgetPath | ConvertFrom-Json
$resolvedBinary = (Resolve-Path $Binary).Path
$start = [Diagnostics.Stopwatch]::StartNew()
$process = Start-Process -FilePath $resolvedBinary -PassThru
try {
    do {
        Start-Sleep -Milliseconds 10
        $process.Refresh()
    } while (-not $process.HasExited -and $process.MainWindowHandle -eq 0 -and $start.ElapsedMilliseconds -lt ($budgets.regression_ceilings.startup_ms + 5000))

    $startupMs = $start.Elapsed.TotalMilliseconds
    if ($process.HasExited) { throw "Process exited before measurement began (exit code $($process.ExitCode))." }
    if ($process.MainWindowHandle -eq 0) { throw "Native window did not become visible within the measurement timeout." }

    Start-Sleep -Seconds $SettleSeconds
    $process.Refresh()
    $settledWorkingSetMb = $process.WorkingSet64 / 1MB
    $peakWorkingSetMb = $settledWorkingSetMb
    $cpuBefore = $process.TotalProcessorTime
    $sampleCount = 0
    $sampleDelayMs = 250
    $sampleStopwatch = [Diagnostics.Stopwatch]::StartNew()
    while ($sampleStopwatch.Elapsed.TotalSeconds -lt $IdleSeconds) {
        Start-Sleep -Milliseconds $sampleDelayMs
        $process.Refresh()
        $peakWorkingSetMb = [Math]::Max($peakWorkingSetMb, $process.WorkingSet64 / 1MB)
        $sampleCount++
    }
    $process.Refresh()
    $idleCpuMs = ($process.TotalProcessorTime - $cpuBefore).TotalMilliseconds

    $result = [ordered]@{
        schema_version = 1
        binary = $resolvedBinary
        measured_at_utc = [DateTime]::UtcNow.ToString('o')
        startup_ms = [Math]::Round($startupMs, 1)
        settled_working_set_mb = [Math]::Round($settledWorkingSetMb, 2)
        peak_working_set_mb = [Math]::Round($peakWorkingSetMb, 2)
        idle_cpu_ms = [Math]::Round($idleCpuMs, 1)
        idle_seconds = $IdleSeconds
        sample_count = $sampleCount
        event_to_visible_ms = if ($PSBoundParameters.ContainsKey('EventToVisibleMs')) { $EventToVisibleMs } else { $null }
        budgets = $budgets
    }
    $result | ConvertTo-Json -Depth 5

    if ($Enforce) {
        $failures = @()
        foreach ($metric in @('startup_ms', 'settled_working_set_mb', 'peak_working_set_mb', 'idle_cpu_ms')) {
            $value = [double]$result[$metric]
            $ceiling = [double]$budgets.regression_ceilings.$metric
            if ($value -gt $ceiling) { $failures += "$metric=$value exceeds ceiling $ceiling" }
        }
        if ($null -ne $EventToVisibleMs -and $EventToVisibleMs -gt $budgets.regression_ceilings.event_to_visible_ms) {
            $failures += "event_to_visible_ms=$EventToVisibleMs exceeds ceiling $($budgets.regression_ceilings.event_to_visible_ms)"
        }
        if ($failures.Count -gt 0) {
            Write-Error ('Performance budget failed: ' + ($failures -join '; '))
            exit 1
        }
    }
}
finally {
    if ($null -ne $process -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force }
}
