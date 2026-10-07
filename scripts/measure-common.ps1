# SPDX-License-Identifier: Apache-2.0
# Helpers shared by the measurement scripts (dot-sourced; not meant to be run alone).

function Resolve-Cockpit {
    param([string]$Exe)
    if ($Exe) {
        if (-not (Test-Path -LiteralPath $Exe)) { throw "The program '$Exe' does not exist." }
        return (Resolve-Path -LiteralPath $Exe).Path
    }
    $root = Split-Path -Parent $PSScriptRoot
    foreach ($kind in 'release', 'debug') {
        $candidate = Join-Path $root "target\$kind\usage-cockpit.exe"
        if (Test-Path -LiteralPath $candidate) {
            if ($kind -eq 'debug') {
                Write-Warning 'Measuring the debug build; build with `cargo build --release` for real numbers.'
            }
            return $candidate
        }
    }
    throw 'No usage-cockpit.exe found; run `cargo build --release` first or pass -Exe.'
}

# A new folder below the temporary folder, used as USAGE_COCKPIT_HOME so that the real data
# and configuration folders are never touched.
function New-TempHome {
    $path = Join-Path ([System.IO.Path]::GetTempPath()) ('usage-cockpit-measure-' + [guid]::NewGuid().ToString('N'))
    [void](New-Item -ItemType Directory -Path $path)
    return $path
}

# Removes a folder made by New-TempHome (and only such a folder).
function Remove-TempHome {
    param([string]$Path)
    if ($Path -and (Split-Path -Leaf $Path) -like 'usage-cockpit-measure-*' -and (Test-Path -LiteralPath $Path)) {
        Remove-Item -LiteralPath $Path -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Get-Median {
    param([double[]]$Sorted)
    $n = $Sorted.Count
    if ($n -eq 0) { return [double]::NaN }
    if ($n % 2 -eq 1) { return $Sorted[($n - 1) / 2] }
    return ($Sorted[$n / 2 - 1] + $Sorted[$n / 2]) / 2
}

# Writes a fixture record (the same one as the bridge measurement) into the data folder by
# running the bridge once, so that the window has values to show.
function Add-FixtureRecord {
    param([string]$Exe, [string]$Home1)
    $now = [DateTimeOffset]::UtcNow
    $record = '{"session_id":"measure","rate_limits":{"five_hour":{"used_percentage":23.5,"resets_at":' +
        $now.AddHours(3).ToUnixTimeSeconds() + '},"seven_day":{"used_percentage":41.2,"resets_at":' +
        $now.AddDays(3).ToUnixTimeSeconds() + '}}}'
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $Exe
    $info.Arguments = 'bridge'
    $info.UseShellExecute = $false
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.CreateNoWindow = $true
    $info.EnvironmentVariables['USAGE_COCKPIT_HOME'] = $Home1
    $process = [System.Diagnostics.Process]::Start($info)
    $process.StandardInput.Write($record)
    $process.StandardInput.Close()
    [void]$process.StandardOutput.ReadToEnd()
    $process.WaitForExit()
}
