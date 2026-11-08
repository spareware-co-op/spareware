# How busy the reference machine is, sampled once a second for a night's record (piece G107;
# chapter 23.13 of the kernel architecture). Windows PowerShell 5.1, started in the background by
# nightly.sh through WSL's interop.
#
# Usage: busy.ps1 <output file> <stop file> <deadline, milliseconds since 1970>
#
# Appends one line a second to the output file, `<milliseconds since 1970, UTC> <percent>`: the
# share of all logical processors' time spent busy over the second before, rounded to a whole
# percent, everything counted (WSL 2's virtual processors run as host threads, so work inside WSL
# counts too). It stops when the stop file exists or at the deadline, whichever comes first.
#
# The counters are Win32_PerfRawData_PerfOS_Processor's raw PercentProcessorTime (idle time in
# 100 ns units, a PERF_100NSEC_TIMER_INV counter) and Timestamp_Sys100NS for each logical
# processor, whose names WMI does not translate; busy = 1 - (idle time) / (elapsed time x
# processors) between two samples (https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-server-2003/cc785636(v=ws.10),
# PERF_100NSEC_TIMER_INV).

param(
    [Parameter(Mandatory = $true)][string]$Output,
    [Parameter(Mandatory = $true)][string]$Stop,
    [Parameter(Mandatory = $true)][long]$Deadline
)

$ErrorActionPreference = 'Stop'

function Sample {
    $all = @(Get-CimInstance -ClassName Win32_PerfRawData_PerfOS_Processor | Where-Object { $_.Name -ne '_Total' })
    [pscustomobject]@{
        Idle  = [double](($all | Measure-Object -Property PercentProcessorTime -Sum).Sum)
        Time  = [double]$all[0].Timestamp_Sys100NS
        Count = $all.Count
    }
}

$before = Sample
while (-not (Test-Path -LiteralPath $Stop) -and [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() -lt $Deadline) {
    Start-Sleep -Milliseconds 1000
    $now = Sample
    $elapsed = ($now.Time - $before.Time) * $now.Count
    if ($elapsed -gt 0) {
        $busy = [math]::Round(100 * (1 - ($now.Idle - $before.Idle) / $elapsed))
        $busy = [math]::Min(100, [math]::Max(0, $busy))
        Add-Content -LiteralPath $Output -Value "$([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()) $busy" -Encoding ascii
    }
    $before = $now
}
