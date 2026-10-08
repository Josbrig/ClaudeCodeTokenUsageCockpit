# SPDX-License-Identifier: Apache-2.0
<#
.SYNOPSIS
  Checks, and on request installs, the tools needed to develop usage-cockpit on Windows.

.DESCRIPTION
  Needed: Rust (the toolchain pinned in rust-toolchain.toml, with rustfmt and clippy) through
  rustup, the Microsoft C++ Build Tools (the linker of the MSVC toolchain), Git for Windows
  (its bash is also used by some tests), CMake and Ninja (for the CMake builds).

  The script first looks at what is there and prints a table. With -CheckOnly it stops there and
  installs nothing. Otherwise it says what it would install, asks, and installs only what is
  missing (winget is used; rustup installs the toolchain). It can be run again at any time. It
  contains no secrets. Programs are installed for the current user where the package allows
  it; the Microsoft C++ Build Tools can only be installed for the whole computer and ask for
  administrator consent (a Windows prompt appears).

.PARAMETER CheckOnly
  Only report; install nothing.

.PARAMETER Yes
  Do not ask before installing.

.PARAMETER SkipBuild
  Do not run `cargo build` at the end to prove that the setup works.

.EXAMPLE
  powershell -File scripts\setup-dev-windows.ps1 -CheckOnly

.EXAMPLE
  powershell -File scripts\setup-dev-windows.ps1
#>
[CmdletBinding()]
param(
    [switch]$CheckOnly,
    [switch]$Yes,
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'

# ---------------------------------------------------------------- pure helpers (tested)

# The channel named in rust-toolchain.toml, for example 1.99.0; $null if there is none.
function Get-ToolchainChannel {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return $null }
    $text = Get-Content -LiteralPath $Path -Raw
    $m = [regex]::Match($text, '(?m)^\s*channel\s*=\s*"([^"]+)"')
    if ($m.Success) { return $m.Groups[1].Value }
    return $null
}

# The first line of a version output, trimmed; $null for nothing.
function Get-VersionLine {
    param([string[]]$Output)
    foreach ($line in $Output) {
        if ($line -and $line.Trim()) { return $line.Trim() }
    }
    return $null
}

# What has to be done for the given tool states: the tools that are not present, in order.
function Get-Plan {
    param([object[]]$States)
    return @($States | Where-Object { -not $_.Present })
}

# The text of the table of states.
function Format-States {
    param([object[]]$States)
    $lines = foreach ($s in $States) {
        $mark = if ($s.Present) { 'ok     ' } else { 'MISSING' }
        $what = if ($s.Present) { $s.Version } else { $s.Hint }
        '{0}  {1,-28} {2}' -f $mark, $s.Name, $what
    }
    return ($lines -join [Environment]::NewLine)
}

# The folders of a stored PATH text (as Windows keeps it for the user or the computer): split at
# `;`, environment variables such as %USERPROFILE% expanded, empty parts and duplicates dropped.
function Get-StoredPathDirs {
    param([string[]]$Stored)
    $dirs = New-Object System.Collections.Generic.List[string]
    foreach ($text in $Stored) {
        if (-not $text) { continue }
        foreach ($part in ($text -split ';')) {
            $expanded = [Environment]::ExpandEnvironmentVariables($part.Trim().Trim('"'))
            if ($expanded -and -not $dirs.Contains($expanded)) { $dirs.Add($expanded) }
        }
    }
    return $dirs.ToArray()
}

# The winget command line (as an argument list) that installs a tool.
function Get-WingetArguments {
    param([string]$Id, [string]$Override, [switch]$UserScope)
    $arguments = @('install', '--id', $Id, '--exact', '--accept-source-agreements', '--accept-package-agreements')
    if ($UserScope) { $arguments += @('--scope', 'user') }
    if ($Override) { $arguments += @('--override', $Override) }
    return $arguments
}

# ---------------------------------------------------------------- looking at the computer

$RepoRoot = Split-Path -Parent $PSScriptRoot

# The places where rustup puts its programs (the user PATH may not have them yet).
function Get-CargoBin {
    $cargoHome = $env:CARGO_HOME
    if (-not $cargoHome) { $cargoHome = Join-Path $env:USERPROFILE '.cargo' }
    return (Join-Path $cargoHome 'bin')
}

function Find-Program {
    param([string]$Name)
    $command = Get-Command $Name -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($command) { return $command.Source }
    # The PATH of this terminal is older than the one Windows has stored if a tool was installed
    # after the terminal was opened (also by this script): look at the stored one as well.
    # (the computer's PATH comes first in Windows, then the user's)
    $stored = Get-StoredPathDirs @(
        [Environment]::GetEnvironmentVariable('Path', 'Machine'),
        [Environment]::GetEnvironmentVariable('Path', 'User')
    )
    # Only programs are taken from the stored folders (not a file without an extension).
    foreach ($dir in (@((Get-CargoBin)) + $stored)) {
        foreach ($candidate in @("$Name.exe", "$Name.cmd", "$Name.bat")) {
            $path = Join-Path $dir $candidate
            if (Test-Path -LiteralPath $path -PathType Leaf) { return $path }
        }
    }
    return $null
}

function Get-ProgramVersion {
    param([string]$Name, [string[]]$Arguments = @('--version'))
    $path = Find-Program $Name
    if (-not $path) { return $null }
    # Some programs (rustup) write notes to the error stream; that is no failure here.
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $output = & $path @Arguments 2>&1 | ForEach-Object { "$_" }
        return (Get-VersionLine $output)
    } catch {
        return $null
    } finally {
        $ErrorActionPreference = $previous
    }
}

function Get-VisualCppVersion {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere)) { return $null }
    $found = & $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property displayName 2>$null
    return (Get-VersionLine @($found))
}

function Get-ToolchainState {
    param([string]$Channel)
    $rustup = Find-Program 'rustup'
    if (-not $rustup) { return $null }
    if (-not $Channel) { return (Get-ProgramVersion 'rustup') }
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $installed = @(& $rustup toolchain list 2>&1 | ForEach-Object { "$_" })
    $match = $installed | Where-Object { $_ -like "$Channel-*-windows-msvc*" } | Select-Object -First 1
    if (-not $match) { return $null }
    # rustfmt and clippy must be there as well
    $components = @(& $rustup component list --toolchain $Channel --installed 2>&1 | ForEach-Object { "$_" })
    $hasFmt = [bool]($components | Where-Object { $_ -like 'rustfmt*' })
    $hasClippy = [bool]($components | Where-Object { $_ -like 'clippy*' })
    $ErrorActionPreference = $previous
    if ($hasFmt -and $hasClippy) { return ($match -replace ' \(.*$', '') + ' with rustfmt and clippy' }
    return $null
}

function Get-States {
    param([string]$Channel)
    $channelText = if ($Channel) { $Channel } else { 'stable' }
    $states = @(
        [pscustomobject]@{
            Name = 'rustup'; Present = [bool](Find-Program 'rustup')
            Version = (Get-ProgramVersion 'rustup'); Hint = 'install with winget (Rustlang.Rustup), without a default toolchain'
            WingetId = 'Rustlang.Rustup'; Override = '-y --default-toolchain none --profile minimal'; Kind = 'winget'
            Program = 'rustup'; PerUser = $true
        },
        [pscustomobject]@{
            Name = "Rust toolchain $channelText"; Present = $false
            Version = $null; Hint = 'install with rustup (with rustfmt and clippy)'
            WingetId = $null; Override = $null; Kind = 'toolchain'; Program = $null; PerUser = $false
        },
        [pscustomobject]@{
            Name = 'Microsoft C++ Build Tools'; Present = $false
            Version = $null; Hint = 'install with winget (Microsoft.VisualStudio.2022.BuildTools, workload C++)'
            WingetId = 'Microsoft.VisualStudio.2022.BuildTools'
            Override = '--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
            Kind = 'winget'; Program = $null; PerUser = $false
        },
        [pscustomobject]@{
            Name = 'Git'; Present = [bool](Find-Program 'git')
            Version = (Get-ProgramVersion 'git'); Hint = 'install with winget (Git.Git)'
            WingetId = 'Git.Git'; Override = $null; Kind = 'winget'; Program = 'git'; PerUser = $true
        },
        [pscustomobject]@{
            Name = 'CMake'; Present = [bool](Find-Program 'cmake')
            Version = (Get-ProgramVersion 'cmake'); Hint = 'install with winget (Kitware.CMake)'
            WingetId = 'Kitware.CMake'; Override = $null; Kind = 'winget'; Program = 'cmake'; PerUser = $true
        },
        [pscustomobject]@{
            Name = 'Ninja'; Present = [bool](Find-Program 'ninja')
            Version = (Get-ProgramVersion 'ninja'); Hint = 'install with winget (Ninja-build.Ninja)'
            WingetId = 'Ninja-build.Ninja'; Override = $null; Kind = 'winget'; Program = 'ninja'; PerUser = $true
        }
    )
    $toolchain = Get-ToolchainState $Channel
    $states[1].Present = [bool]$toolchain
    $states[1].Version = $toolchain
    $vc = Get-VisualCppVersion
    $states[2].Present = [bool]$vc
    $states[2].Version = $vc
    return $states
}

# ---------------------------------------------------------------- installing

function Confirm-Install {
    param([string]$Question)
    if ($Yes) { return $true }
    if (-not [Environment]::UserInteractive -or [Console]::IsInputRedirected) {
        Write-Host 'No question can be asked here; run the script in a terminal or add -Yes.'
        return $false
    }
    $answer = Read-Host "$Question [y/N]"
    return ($answer -match '^(y|yes)$')
}

function Install-WithWinget {
    param([object]$Tool)
    $winget = Get-Command winget -ErrorAction SilentlyContinue
    if (-not $winget) {
        throw "winget is not available. Install '$($Tool.Name)' by hand (id $($Tool.WingetId)), then run this script again."
    }
    # For the current user first, where the package allows it; else for the whole computer.
    $code = 1
    if ($Tool.PerUser) {
        $arguments = Get-WingetArguments -Id $Tool.WingetId -Override $Tool.Override -UserScope
        Write-Host "winget $($arguments -join ' ')"
        & winget @arguments | Out-Host
        $code = $LASTEXITCODE
    }
    if ($code -ne 0) {
        $arguments = Get-WingetArguments -Id $Tool.WingetId -Override $Tool.Override
        Write-Host "winget $($arguments -join ' ')"
        & winget @arguments | Out-Host
        $code = $LASTEXITCODE
    }
    if ($code -ne 0) {
        # winget also reports "already installed" with a code; a program that is there is fine.
        if ($Tool.Program -and (Find-Program $Tool.Program)) { return }
        throw "winget could not install $($Tool.Name) (exit code $code). If it is installed but not on the PATH, open a new terminal and run this script again."
    }
}

function Install-Toolchain {
    param([string]$Channel)
    $rustup = Find-Program 'rustup'
    if (-not $rustup) { throw 'rustup is not installed yet; install it first (open a new terminal after that).' }
    $channelText = if ($Channel) { $Channel } else { 'stable' }
    Write-Host "rustup toolchain install $channelText --profile minimal -c rustfmt -c clippy"
    & $rustup toolchain install $channelText --profile minimal -c rustfmt -c clippy | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "rustup could not install the toolchain (exit code $LASTEXITCODE)." }
}

# The exit code of the script. It is a variable and not a return value, because the output of
# the programs that are started would become part of a return value.
$script:ExitCode = 0

function Invoke-Setup {
    $script:ExitCode = 0
    $channel = Get-ToolchainChannel (Join-Path $RepoRoot 'rust-toolchain.toml')
    $states = Get-States $channel
    Write-Host 'Development tools for usage-cockpit'
    Write-Host (Format-States $states)
    $plan = @(Get-Plan $states)
    if ($plan.Count -eq 0) {
        Write-Host 'Everything is there.'
    } elseif ($CheckOnly) {
        Write-Host ''
        Write-Host "$($plan.Count) missing. Nothing was installed (-CheckOnly). Run without -CheckOnly to install."
        $script:ExitCode = 1
        return
    } else {
        Write-Host ''
        Write-Host 'This would be installed:'
        foreach ($tool in $plan) { Write-Host "  - $($tool.Name): $($tool.Hint)" }
        if (-not (Confirm-Install 'Install the missing tools?')) {
            Write-Host 'Nothing was installed.'
            $script:ExitCode = 1
            return
        }
        foreach ($tool in $plan) {
            if ($tool.Kind -eq 'toolchain') { continue }
            Install-WithWinget $tool
        }
        if ($plan | Where-Object { $_.Kind -eq 'toolchain' -or $_.Name -eq 'rustup' }) {
            Install-Toolchain $channel
        }
        Write-Host ''
        Write-Host 'Done. Open a new terminal so that the PATH of the new tools is in effect.'
    }
    if (-not $SkipBuild -and -not $CheckOnly) {
        $cargoBin = Get-CargoBin
        if (Test-Path -LiteralPath $cargoBin) { $env:PATH = "$cargoBin;$env:PATH" }
        $cargo = Find-Program 'cargo'
        if ($cargo) {
            Write-Host ''
            Write-Host 'Building once to prove that the setup works (cargo build) ...'
            Push-Location $RepoRoot
            try {
                & $cargo build | Out-Host
                if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit code $LASTEXITCODE)." }
            } finally { Pop-Location }
            Write-Host 'The build works.'
        } else {
            Write-Host ''
            Write-Host 'cargo was not found on this PATH, so the build was skipped. Open a new terminal and run the script again to check it.'
        }
    }
    Write-Host ''
    Write-Host 'Build and test:'
    Write-Host '  cargo fmt --all --check'
    Write-Host '  cargo clippy --all-targets -- -D warnings'
    Write-Host '  cargo test --all'
}

# Run only when the script is started, not when it is read by the tests (dot-sourced).
if ($MyInvocation.InvocationName -ne '.') {
    try {
        Invoke-Setup
    } catch {
        Write-Host "ERROR: $($_.Exception.Message)"
        $script:ExitCode = 2
    }
    exit $script:ExitCode
}
