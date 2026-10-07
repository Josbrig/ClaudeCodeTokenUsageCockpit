# SPDX-License-Identifier: Apache-2.0
<#
.SYNOPSIS
  Measures CPU and memory of the running cockpit while it is idle (REQ-105).

.DESCRIPTION
  Starts the cockpit with a fixture record in a temporary data folder, waits 15 seconds for the
  start-up to calm down, then samples once a second for 10 minutes (or -Seconds): the share of
  one CPU core and the working set in MB. Prints the averages and the maxima, then ends the
  cockpit. Leave the machine alone during the measurement; do not use the cockpit window.

.PARAMETER Exe
  The program to measure. Default: the release build, else the debug build, in this repository.

.PARAMETER Seconds
  Length of the measurement. Default 600 (10 minutes).

.EXAMPLE
  powershell -File scripts\measure-idle.ps1 -Seconds 60
#>
[CmdletBinding()]
param(
    [string]$Exe,
    [int]$Seconds = 600
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'measure-common.ps1')
$Exe = Resolve-Cockpit -Exe $Exe

$home1 = New-TempHome
$process = $null
try {
    Add-FixtureRecord -Exe $Exe -Home1 $home1
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $Exe
    $info.UseShellExecute = $false
    $info.EnvironmentVariables['USAGE_COCKPIT_HOME'] = $home1
    $process = [System.Diagnostics.Process]::Start($info)
    Start-Sleep -Seconds 15
    if ($process.HasExited) { throw "The cockpit ended by itself with exit code $($process.ExitCode)." }

    $cpuShare = New-Object System.Collections.Generic.List[double]
    $memoryMb = New-Object System.Collections.Generic.List[double]
    $process.Refresh()
    $lastCpu = $process.TotalProcessorTime.TotalSeconds
    $lastTime = [System.Diagnostics.Stopwatch]::GetTimestamp()
    for ($i = 0; $i -lt $Seconds; $i++) {
        Start-Sleep -Seconds 1
        $process.Refresh()
        if ($process.HasExited) { throw "The cockpit ended after $i seconds." }
        $cpu = $process.TotalProcessorTime.TotalSeconds
        $time = [System.Diagnostics.Stopwatch]::GetTimestamp()
        $elapsed = ($time - $lastTime) / [System.Diagnostics.Stopwatch]::Frequency
        # Share of ONE core in percent: a program that keeps one core busy shows 100.
        $cpuShare.Add(100.0 * ($cpu - $lastCpu) / $elapsed)
        $memoryMb.Add($process.WorkingSet64 / 1MB)
        $lastCpu = $cpu
        $lastTime = $time
    }
    '{0} s idle: CPU average {1:N2} % of one core (maximum {2:N2} %), memory average {3:N1} MB (maximum {4:N1} MB)' -f `
        $Seconds, ($cpuShare | Measure-Object -Average).Average, ($cpuShare | Measure-Object -Maximum).Maximum, `
        ($memoryMb | Measure-Object -Average).Average, ($memoryMb | Measure-Object -Maximum).Maximum
    'Limits: CPU below 1 %, memory below 100 MB.'
}
finally {
    if ($process -and -not $process.HasExited) { $process.Kill(); [void]$process.WaitForExit(5000) }
    Remove-TempHome $home1
}
