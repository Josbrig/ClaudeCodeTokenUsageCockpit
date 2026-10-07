# SPDX-License-Identifier: Apache-2.0
<#
.SYNOPSIS
  Tests of the pure parts of scripts\setup-dev-windows.ps1 (no installation is started).

.EXAMPLE
  powershell -File scripts\tests\setup-dev-windows.tests.ps1
#>
$ErrorActionPreference = 'Stop'
. (Join-Path (Split-Path -Parent $PSScriptRoot) 'setup-dev-windows.ps1')

$failed = 0
function Assert-Equal {
    param($Actual, $Expected, [string]$Name)
    if ($Actual -ceq $Expected) {
        Write-Host "ok    $Name"
    } else {
        Write-Host "FAIL  $Name"
        Write-Host "      expected: $Expected"
        Write-Host "      actual:   $Actual"
        $script:failed++
    }
}

# ---- the channel of the toolchain file
$temp = Join-Path ([IO.Path]::GetTempPath()) ('cockpit-setup-test-' + [guid]::NewGuid().ToString('N'))
[void](New-Item -ItemType Directory $temp)
try {
    $file = Join-Path $temp 'rust-toolchain.toml'
    Set-Content -LiteralPath $file -Value "[toolchain]`nchannel = `"1.99.0`"`nprofile = `"minimal`"`n"
    Assert-Equal (Get-ToolchainChannel $file) '1.99.0' 'channel is read from rust-toolchain.toml'
    Set-Content -LiteralPath $file -Value "[toolchain]`n# channel = `"nightly`"`nprofile = `"minimal`"`n"
    Assert-Equal ([string](Get-ToolchainChannel $file)) '' 'a commented channel is not read'
    Assert-Equal ([string](Get-ToolchainChannel (Join-Path $temp 'missing.toml'))) '' 'a missing file gives no channel'
} finally {
    Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
}
$real = Get-ToolchainChannel (Join-Path $RepoRoot 'rust-toolchain.toml')
Assert-Equal ([bool]$real) $true 'the real rust-toolchain.toml names a channel'

# ---- version lines
Assert-Equal (Get-VersionLine @('', '  git version 2.50.0  ', 'x')) 'git version 2.50.0' 'the first non-empty line is the version'
Assert-Equal ([string](Get-VersionLine @())) '' 'no output gives no version'

# ---- the plan
$states = @(
    [pscustomobject]@{ Name = 'a'; Present = $true; Version = '1'; Hint = 'h' },
    [pscustomobject]@{ Name = 'b'; Present = $false; Version = $null; Hint = 'install b' },
    [pscustomobject]@{ Name = 'c'; Present = $false; Version = $null; Hint = 'install c' }
)
$plan = Get-Plan $states
Assert-Equal $plan.Count 2 'the plan holds the missing tools'
Assert-Equal (($plan | ForEach-Object { $_.Name }) -join ',') 'b,c' 'the plan keeps the order'
Assert-Equal @(Get-Plan @($states[0])).Count 0 'nothing is missing'
Assert-Equal @(Get-Plan @($states[1])).Count 1 'a single missing tool is a plan of one'

# ---- the table
$text = Format-States $states
Assert-Equal ($text -match 'ok\s+a\s+1') $true 'a present tool shows its version'
Assert-Equal ($text -match 'MISSING\s+b\s+install b') $true 'a missing tool shows what would be done'

# ---- the winget arguments
Assert-Equal ((Get-WingetArguments -Id 'Git.Git') -join ' ') 'install --id Git.Git --exact --accept-source-agreements --accept-package-agreements' 'winget arguments without an override'
$withOverride = Get-WingetArguments -Id 'X.Y' -Override '--passive'
Assert-Equal ($withOverride[-2..-1] -join ' ') '--override --passive' 'winget arguments with an override'

# ---- the real computer: the check runs and every state has a name
$real = Get-States $real
Assert-Equal ($real.Count -ge 6) $true 'the real check lists all tools'
Assert-Equal (@($real | Where-Object { -not $_.Name }).Count) 0 'every tool has a name'

if ($failed -gt 0) { Write-Host "$failed test(s) failed"; exit 1 }
Write-Host 'all tests passed'
