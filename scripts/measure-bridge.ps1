# SPDX-License-Identifier: Apache-2.0
<#
.SYNOPSIS
  Measures how long the status line bridge needs for one call (REQ-109).

.DESCRIPTION
  Runs `usage-cockpit bridge` 100 times (or -Runs times) with a fixture record on standard input
  and prints the median, the 95th percentile and the maximum of the wall time in milliseconds.
  The data and configuration folders are redirected into a temporary folder, so nothing of the
  real installation is touched.

  The wall time includes the start of the process, which a PowerShell script cannot separate
  from the program's own work. To show how much of it is the start of any process on this
  machine, the script also runs an empty program (`cmd.exe /c exit`) the same way and prints its
  median. The difference is the part that belongs to the bridge. The requirement (100 ms)
  concerns the bridge's own processing; both numbers are printed so that the reader can judge.

.PARAMETER Exe
  The program to measure. Default: the release build, else the debug build, in this repository.

.PARAMETER Runs
  Number of calls. Default 100.

.EXAMPLE
  powershell -File scripts\measure-bridge.ps1
#>
[CmdletBinding()]
param(
    [string]$Exe,
    [int]$Runs = 100
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'measure-common.ps1')
$Exe = Resolve-Cockpit -Exe $Exe

# Wall time in milliseconds of one call of $Program with $Arguments and $Input on standard input.
function Measure-Call {
    param([string]$Program, [string]$Arguments, [string]$InputText, [string]$Home1)
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $Program
    $info.Arguments = $Arguments
    $info.UseShellExecute = $false
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.CreateNoWindow = $true
    $info.EnvironmentVariables['USAGE_COCKPIT_HOME'] = $Home1
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    $process = [System.Diagnostics.Process]::Start($info)
    $process.StandardInput.Write($InputText)
    $process.StandardInput.Close()
    $output = $process.StandardOutput.ReadToEnd()
    $process.WaitForExit()
    $watch.Stop()
    return [pscustomobject]@{ Ms = $watch.Elapsed.TotalMilliseconds; Code = $process.ExitCode; Output = $output }
}

$home1 = New-TempHome
try {
    $now = [DateTimeOffset]::UtcNow
    $record = '{"session_id":"measure","version":"0.0.0","model":{"display_name":"Fixture"},' +
        '"rate_limits":{"five_hour":{"used_percentage":23.5,"resets_at":' + $now.AddHours(3).ToUnixTimeSeconds() + '},' +
        '"seven_day":{"used_percentage":41.2,"resets_at":' + $now.AddDays(3).ToUnixTimeSeconds() + '}}}'

    $times = New-Object System.Collections.Generic.List[double]
    for ($i = 0; $i -lt $Runs; $i++) {
        $call = Measure-Call -Program $Exe -Arguments 'bridge' -InputText $record -Home1 $home1
        if ($call.Code -ne 0 -or [string]::IsNullOrWhiteSpace($call.Output)) {
            throw "Run $($i + 1): exit code $($call.Code), output '$($call.Output)'."
        }
        $times.Add($call.Ms)
    }
    $baseline = New-Object System.Collections.Generic.List[double]
    $cmd = Join-Path $env:SystemRoot 'System32\cmd.exe'
    for ($i = 0; $i -lt [math]::Min($Runs, 50); $i++) {
        $baseline.Add((Measure-Call -Program $cmd -Arguments '/c exit' -InputText '' -Home1 $home1).Ms)
    }

    $sorted = $times | Sort-Object
    $median = Get-Median $sorted
    $p95 = $sorted[[math]::Min($sorted.Count - 1, [int][math]::Ceiling($sorted.Count * 0.95) - 1)]
    $max = $sorted[$sorted.Count - 1]
    $empty = Get-Median ($baseline | Sort-Object)
    '{0} runs: median {1:N1} ms, 95th percentile {2:N1} ms, maximum {3:N1} ms' -f $Runs, $median, $p95, $max
    'An empty program (cmd.exe /c exit) takes {0:N1} ms the same way; bridge minus empty program: {1:N1} ms (limit: 100 ms)' -f `
        $empty, ($median - $empty)
}
finally {
    Remove-TempHome $home1
}
