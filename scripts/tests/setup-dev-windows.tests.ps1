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

# ---- the stored PATH
$env:COCKPIT_TEST_DIR = 'C:\Tools\X'
$dirs = Get-StoredPathDirs @('C:\A;"C:\B B";;%COCKPIT_TEST_DIR%\bin', 'C:\A;C:\C', $null)
Assert-Equal ($dirs -join '|') 'C:\A|C:\B B|C:\Tools\X\bin|C:\C' 'the stored PATH is split, expanded and without duplicates'
Remove-Item Env:COCKPIT_TEST_DIR
Assert-Equal @(Get-StoredPathDirs @()).Count 0 'no stored PATH gives no folders'

# a program that is only on the stored PATH is found (a folder with a fake program)
$fakeDir = Join-Path ([IO.Path]::GetTempPath()) ('cockpit-fake-tool-' + [guid]::NewGuid().ToString('N'))
[void](New-Item -ItemType Directory $fakeDir)
try {
    Set-Content -LiteralPath (Join-Path $fakeDir 'cockpit-fake-tool.exe') -Value 'x'
    Assert-Equal ([string](Find-Program 'cockpit-fake-tool')) '' 'a program outside every PATH is not found'
    $before = [Environment]::GetEnvironmentVariable('Path', 'Process')
    # the folder is not on the PATH of this terminal; Find-Program must look at the stored PATH,
    # which the function reads through this seam
    function Get-StoredPathDirs { param([string[]]$Stored) return @($fakeDir) }
    Assert-Equal ((Find-Program 'cockpit-fake-tool') -like '*cockpit-fake-tool.exe') $true 'a program on the stored PATH is found'
    Assert-Equal ([Environment]::GetEnvironmentVariable('Path', 'Process')) $before 'the PATH of this terminal is not changed'
} finally {
    Remove-Item -LiteralPath $fakeDir -Recurse -Force -ErrorAction SilentlyContinue
    . (Join-Path (Split-Path -Parent $PSScriptRoot) 'setup-dev-windows.ps1')
}

# ---- the winget scope
Assert-Equal ((Get-WingetArguments -Id 'X.Y' -UserScope) -join ' ') 'install --id X.Y --exact --accept-source-agreements --accept-package-agreements --scope user' 'winget arguments for the current user'

# ---- the flow of the script with the installers replaced by fakes (nothing is installed)
function New-FakeStates {
    param([bool[]]$Present)
    $names = 'rustup', 'Rust toolchain', 'Microsoft C++ Build Tools', 'Git', 'CMake', 'Ninja'
    for ($i = 0; $i -lt $names.Count; $i++) {
        [pscustomobject]@{
            Name = $names[$i]; Present = $Present[$i]; Version = 'v'; Hint = "install $($names[$i])"
            WingetId = "Id.$i"; Override = $null; Kind = if ($i -eq 1) { 'toolchain' } else { 'winget' }
            Program = $null; PerUser = $true
        }
    }
}
$script:installed = @()
function Install-WithWinget { param([object]$Tool) $script:installed += $Tool.Name }
function Install-Toolchain { param([string]$Channel) $script:installed += 'toolchain' }

$SkipBuild = $true
$allThere = @($true, $true, $true, $true, $true, $true)
$oneMissing = @($true, $true, $true, $true, $true, $false)

function Get-States { param([string]$Channel) return (New-FakeStates $script:fake) }

$script:fake = $allThere
$CheckOnly = $false; $Yes = $false
Invoke-Setup | Out-Null
Assert-Equal $script:ExitCode 0 'everything there: exit code 0'
Assert-Equal $script:installed.Count 0 'everything there: nothing is installed'

$script:fake = $oneMissing
$CheckOnly = $true
Invoke-Setup | Out-Null
Assert-Equal $script:ExitCode 1 'check only with a missing tool: exit code 1'
Assert-Equal $script:installed.Count 0 'check only installs nothing'

$CheckOnly = $false; $Yes = $false
function Confirm-Install { param([string]$Question) return $false }
Invoke-Setup | Out-Null
Assert-Equal $script:ExitCode 1 'declined: exit code 1'
Assert-Equal $script:installed.Count 0 'declined installs nothing'

function Confirm-Install { param([string]$Question) return $true }
Invoke-Setup | Out-Null
Assert-Equal $script:ExitCode 0 'agreed: exit code 0 although the installers print'
Assert-Equal ($script:installed -join ',') 'Ninja' 'agreed: only the missing tool is installed'

$script:installed = @()
$script:fake = @($false, $false, $true, $true, $true, $true)
Invoke-Setup | Out-Null
Assert-Equal ($script:installed -join ',') 'rustup,toolchain' 'a missing rustup installs rustup, then the toolchain'

# ---- the real computer: the check runs and every state has a name
Remove-Item function:Get-States
. (Join-Path (Split-Path -Parent $PSScriptRoot) 'setup-dev-windows.ps1')
$real = Get-States $real
Assert-Equal ($real.Count -ge 6) $true 'the real check lists all tools'
Assert-Equal (@($real | Where-Object { -not $_.Name }).Count) 0 'every tool has a name'

if ($failed -gt 0) { Write-Host "$failed test(s) failed"; exit 1 }
Write-Host 'all tests passed'
