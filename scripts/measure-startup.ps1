# SPDX-License-Identifier: Apache-2.0
<#
.SYNOPSIS
  Measures the time from the start of the cockpit until its window is ready (REQ-116).

.DESCRIPTION
  Starts the cockpit 5 times (or -Starts times), each with a fixture record in a fresh temporary
  data folder, and measures the time until the program writes the log line `window ready`. The
  log file is looked at every 20 ms. Prints the time of every start and the maximum, then ends
  the cockpit each time. Do not touch the windows that open.

.PARAMETER Exe
  The program to measure. Default: the release build, else the debug build, in this repository.

.PARAMETER Starts
  Number of starts. Default 5.

.PARAMETER TimeoutSeconds
  How long to wait for each start. Default 20.

.EXAMPLE
  powershell -File scripts\measure-startup.ps1
#>
[CmdletBinding()]
param(
    [string]$Exe,
    [int]$Starts = 5,
    [int]$TimeoutSeconds = 20
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'measure-common.ps1')
$Exe = Resolve-Cockpit -Exe $Exe

$times = New-Object System.Collections.Generic.List[double]
for ($i = 1; $i -le $Starts; $i++) {
    $home1 = New-TempHome
    $process = $null
    try {
        Add-FixtureRecord -Exe $Exe -Home1 $home1
        $log = Join-Path $home1 'data\log.txt'
        $info = New-Object System.Diagnostics.ProcessStartInfo
        $info.FileName = $Exe
        $info.UseShellExecute = $false
        $info.EnvironmentVariables['USAGE_COCKPIT_HOME'] = $home1
        $watch = [System.Diagnostics.Stopwatch]::StartNew()
        $process = [System.Diagnostics.Process]::Start($info)
        $ready = $false
        while ($watch.Elapsed.TotalSeconds -lt $TimeoutSeconds) {
            if (Test-Path -LiteralPath $log) {
                $stream = $null
                try {
                    $stream = [System.IO.File]::Open($log, 'Open', 'Read', 'ReadWrite')
                    $reader = New-Object System.IO.StreamReader($stream)
                    $text = $reader.ReadToEnd()
                    if ($text -match 'window ready') { $ready = $true }
                }
                catch { }
                finally { if ($stream) { $stream.Dispose() } }
                if ($ready) { break }
            }
            if ($process.HasExited) { throw "Start ${i}: the cockpit ended with exit code $($process.ExitCode)." }
            Start-Sleep -Milliseconds 20
        }
        $watch.Stop()
        if (-not $ready) { throw "Start ${i}: no 'window ready' line within $TimeoutSeconds seconds." }
        $times.Add($watch.Elapsed.TotalMilliseconds)
        'Start {0}: {1:N0} ms' -f $i, $watch.Elapsed.TotalMilliseconds
    }
    finally {
        if ($process -and -not $process.HasExited) { $process.Kill(); [void]$process.WaitForExit(5000) }
        Remove-TempHome $home1
    }
}
'{0} starts: average {1:N0} ms, maximum {2:N0} ms (limit: 2000 ms)' -f `
    $Starts, ($times | Measure-Object -Average).Average, ($times | Measure-Object -Maximum).Maximum
