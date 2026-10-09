# SPDX-License-Identifier: Apache-2.0
<#
.SYNOPSIS
  Creates a build folder with CMake, asks how to build, and builds everything in the project that
  can be built.

.DESCRIPTION
  What can be built here (CMakeLists.txt): the program usage-cockpit, the test program
  tools/usage-probe, the checks (cargo fmt --check and clippy -D warnings for both), the tests (ctest:
  fmt, clippy and cargo test for both) and the release file (dist: the executable and SHA256SUMS).

  The script asks:
    1. which build tool CMake shall use (Ninja, Visual Studio, ... , only what is installed),
    2. what to build (everything, or a choice of the parts),
    3. what to do with an existing build folder that was made with another tool (CMake cannot
       change the tool of a folder; the script offers to delete that folder and says which one),
  shows the plan and asks once more before it starts. Every question has a parameter, so that
  nothing is asked with -Yes. The program is built with the release profile of cargo whatever the
  build tool says; the output goes to the build folder (cargo-target, cargo-target-probe, dist).

  It changes nothing outside the build folder. Exit codes: 0 all steps ok, 1 a step failed,
  2 a tool is missing, wrong parameters or cancelled.

.PARAMETER BuildFolder
  The build folder, relative to the repository or absolute and inside it. Default: build.

.PARAMETER Generator
  The CMake generator by name (for example Ninja). Asked when missing, unless -Yes (then Ninja if it
  is installed, else the default of CMake).

.PARAMETER Parts
  What to build: all, or a comma separated list of app, probe, check, test, dist. Asked when
  missing, unless -Yes (then all).

.PARAMETER Yes
  Ask nothing; use the parameters, else the defaults. An existing folder of another tool is
  deleted only together with -ReplaceFolder.

.PARAMETER ReplaceFolder
  Delete an existing build folder that was made with another generator without asking.

.PARAMETER PlanOnly
  Show the plan and stop; run nothing.

.EXAMPLE
  powershell -File scripts\build.ps1

.EXAMPLE
  powershell -File scripts\build.ps1 -Yes -Generator Ninja -Parts app,probe
#>
[CmdletBinding()]
param(
    [string]$BuildFolder = 'build',
    [string]$Generator,
    [string]$Parts,
    [switch]$Yes,
    [switch]$ReplaceFolder,
    [switch]$PlanOnly
)

$ErrorActionPreference = 'Stop'
$RepoRoot = Split-Path -Parent $PSScriptRoot

# All parts, in the order in which they are run, with a text for the menu.
$script:AllParts = @(
    [pscustomobject]@{ Key = 'app';   Text = 'the program usage-cockpit (cargo build --release)' },
    [pscustomobject]@{ Key = 'probe'; Text = 'the test program tools/usage-probe' },
    [pscustomobject]@{ Key = 'check'; Text = 'the checks: cargo fmt --check and clippy -D warnings' },
    [pscustomobject]@{ Key = 'test';  Text = 'the tests (ctest: fmt, clippy, cargo test, for both projects)' },
    [pscustomobject]@{ Key = 'dist';  Text = 'the release file with SHA256SUMS (dist)' }
)

# Where answers come from; the tests replace it.
$script:Ask = { param([string]$Prompt) Read-Host $Prompt }

# ---------------------------------------------------------------- pure helpers (tested)

# The generators in the text of `cmake --help`: objects with Name and IsDefault (the one with *).
function Get-CMakeGenerators {
    param([string[]]$HelpText)
    $found = @()
    $inside = $false
    foreach ($line in $HelpText) {
        if ($line -match '^\s*Generators\s*$') { $inside = $true; continue }
        if (-not $inside) { continue }
        $m = [regex]::Match($line, '^(\*?)\s{0,2}(\S.*?)\s+=\s+\S')
        if ($m.Success -and $line -match '^[\* ] ') {
            $name = $m.Groups[2].Value.Trim()
            # a wrapped description continues on a line that has no " = "
            $found += [pscustomobject]@{ Name = $name; IsDefault = ($m.Groups[1].Value -eq '*') }
        }
    }
    return $found
}

# The generators to offer: [pscustomobject] Name, Text, Recommended. Only what can work here.
# $Have says which programs were found: ninja, nmake, make (hashtable of booleans).
function Get-GeneratorChoices {
    param($Generators, [hashtable]$Have)
    $names = @($Generators | ForEach-Object { $_.Name })
    $choices = @()
    if ($Have.ninja -and ($names -contains 'Ninja')) {
        $choices += [pscustomobject]@{ Name = 'Ninja'; Text = 'Ninja (fast, clean build folder; recommended for VS Code)'; Recommended = $true }
    }
    $default = $Generators | Where-Object { $_.IsDefault } | Select-Object -First 1
    if ($default -and $default.Name -like 'Visual Studio*') {
        $choices += [pscustomobject]@{ Name = $default.Name; Text = "$($default.Name) (project files for Visual Studio; the default of CMake here)"; Recommended = $false }
    } elseif ($default -and $default.Name -ne 'Ninja') {
        $choices += [pscustomobject]@{ Name = $default.Name; Text = "$($default.Name) (the default of CMake here)"; Recommended = $false }
    }
    if ($Have.nmake -and ($names -contains 'NMake Makefiles')) {
        $choices += [pscustomobject]@{ Name = 'NMake Makefiles'; Text = 'NMake Makefiles (needs the Developer Command Prompt)'; Recommended = $false }
    }
    if ($Have.make -and ($names -contains 'MinGW Makefiles')) {
        $choices += [pscustomobject]@{ Name = 'MinGW Makefiles'; Text = 'MinGW Makefiles'; Recommended = $false }
    }
    if ($Have.make -and ($names -contains 'Unix Makefiles')) {
        $choices += [pscustomobject]@{ Name = 'Unix Makefiles'; Text = 'Unix Makefiles'; Recommended = $false }
    }
    # no double entries
    $seen = @{}
    $unique = @()
    foreach ($c in $choices) {
        if (-not $seen.ContainsKey($c.Name)) { $seen[$c.Name] = $true; $unique += $c }
    }
    return $unique
}

# Asks for one of the options (numbers from 1); an empty answer is the default; q cancels
# ($null). A wrong answer is asked again, three times at most.
function Read-Choice {
    param([string]$Title, [string[]]$Options, [int]$Default = 1)
    Write-Host ''
    Write-Host $Title
    for ($i = 0; $i -lt $Options.Count; $i++) {
        $mark = if (($i + 1) -eq $Default) { ' (default)' } else { '' }
        Write-Host ("  {0}) {1}{2}" -f ($i + 1), $Options[$i], $mark)
    }
    for ($try = 0; $try -lt 3; $try++) {
        $answer = (& $script:Ask "Number [$Default], q to cancel").Trim()
        if ($answer -eq '') { return $Default }
        if ($answer -ieq 'q') { return $null }
        $n = 0
        if ([int]::TryParse($answer, [ref]$n) -and $n -ge 1 -and $n -le $Options.Count) { return $n }
        Write-Host "Please type a number from 1 to $($Options.Count)."
    }
    return $null
}

# The keys of the parts from a text: "all" or "app,probe"; $null for something that is wrong.
function ConvertTo-PartList {
    param([string]$Text, $AllParts)
    $keys = @($AllParts | ForEach-Object { $_.Key })
    $t = ($Text -replace '\s', '').ToLowerInvariant()
    if ($t -eq '' ) { return $null }
    if ($t -eq 'all') { return $keys }
    $asked = $t -split ','
    foreach ($k in $asked) { if ($keys -notcontains $k) { return $null } }
    # in the order of the parts, without double entries
    return @($keys | Where-Object { $asked -contains $_ })
}

# Asks which parts: everything, or a choice by numbers (for example 1,2,4).
function Read-Parts {
    param($AllParts)
    $first = Read-Choice 'What shall be built?' @('everything that can be built', 'let me choose the parts') 1
    if ($null -eq $first) { return $null }
    if ($first -eq 1) { return @($AllParts | ForEach-Object { $_.Key }) }
    Write-Host ''
    Write-Host 'Parts:'
    for ($i = 0; $i -lt $AllParts.Count; $i++) { Write-Host ("  {0}) {1}" -f ($i + 1), $AllParts[$i].Text) }
    for ($try = 0; $try -lt 3; $try++) {
        $answer = (& $script:Ask 'Numbers separated by commas, for example 1,2,4 (q to cancel)').Trim()
        if ($answer -ieq 'q') { return $null }
        $nums = @()
        $ok = $answer -ne ''
        foreach ($piece in ($answer -split ',')) {
            $n = 0
            if ([int]::TryParse($piece.Trim(), [ref]$n) -and $n -ge 1 -and $n -le $AllParts.Count) { $nums += $n } else { $ok = $false }
        }
        if ($ok) {
            return @($AllParts | Where-Object { $nums -contains ([array]::IndexOf($AllParts, $_) + 1) } | ForEach-Object { $_.Key })
        }
        Write-Host "Please type numbers from 1 to $($AllParts.Count) separated by commas."
    }
    return $null
}

# The generator that a build folder was made with (from CMakeCache.txt); $null if unknown.
function Get-CachedGenerator {
    param([string]$Folder)
    $cache = Join-Path $Folder 'CMakeCache.txt'
    if (-not (Test-Path -LiteralPath $cache)) { return $null }
    foreach ($line in Get-Content -LiteralPath $cache) {
        if ($line -match '^CMAKE_GENERATOR:INTERNAL=(.+)$') { return $Matches[1].Trim() }
    }
    return $null
}

# The full path of the build folder, or $null if it is not inside the repository or is the
# repository itself.
function Resolve-BuildFolder {
    param([string]$Folder, [string]$Root)
    if ([string]::IsNullOrWhiteSpace($Folder)) { return $null }
    $path = if ([IO.Path]::IsPathRooted($Folder)) { $Folder } else { Join-Path $Root $Folder }
    $full = [IO.Path]::GetFullPath($path).TrimEnd('\', '/')
    $rootFull = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/')
    if ($full -ieq $rootFull) { return $null }
    if (-not $full.StartsWith($rootFull + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { return $null }
    return $full
}

# True if the folder may be deleted by the script: inside the repository, and it is a CMake build
# folder (it has a CMakeCache.txt) that was made for this repository.
function Test-Deletable {
    param([string]$Full, [string]$Root)
    if (-not $Full) { return $false }
    $cache = Join-Path $Full 'CMakeCache.txt'
    if (-not (Test-Path -LiteralPath $cache)) { return $false }
    $line = Get-Content -LiteralPath $cache | Where-Object { $_ -match '^CMAKE_HOME_DIRECTORY:INTERNAL=' } | Select-Object -First 1
    if (-not $line) { return $false }
    $home1 = ($line -replace '^CMAKE_HOME_DIRECTORY:INTERNAL=', '').Replace('/', '\').TrimEnd('\')
    $rootFull = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/')
    return ($home1 -ieq $rootFull)
}

# The steps to run, in order. Each: Title and the command as a list (program, arguments).
function Get-Steps {
    param([string[]]$PartKeys, [string]$Folder, [string]$GeneratorName, [string]$Root)
    $steps = @()
    $steps += [pscustomobject]@{ Title = "configure with $GeneratorName"; Command = @('cmake', '-S', $Root, '-B', $Folder, '-G', $GeneratorName) }
    $build = { param($target, $title) [pscustomobject]@{ Title = $title; Command = @('cmake', '--build', $Folder, '--config', 'Release', '--target', $target) } }
    if ($PartKeys -contains 'app')   { $steps += & $build 'usage-cockpit' 'build usage-cockpit' }
    if ($PartKeys -contains 'probe') { $steps += & $build 'usage-probe' 'build tools/usage-probe' }
    if ($PartKeys -contains 'check') { $steps += & $build 'check' 'checks (fmt and clippy)' }
    if ($PartKeys -contains 'test')  { $steps += [pscustomobject]@{ Title = 'tests (ctest)'; Command = @('ctest', '--test-dir', $Folder, '-C', 'Release', '--output-on-failure') } }
    if ($PartKeys -contains 'dist')  { $steps += & $build 'dist' 'release file (dist)' }
    return $steps
}

# One line per step for the plan.
function Format-Plan {
    param($Steps)
    $i = 0
    return @($Steps | ForEach-Object { $i++; ("  {0}. {1}`n       {2}" -f $i, $_.Title, ($_.Command -join ' ')) })
}

# ---------------------------------------------------------------- tools and the run

function Get-StoredPathDirs {
    $dirs = @()
    foreach ($scope in 'User', 'Machine') {
        $p = [Environment]::GetEnvironmentVariable('Path', $scope)
        if ($p) { $dirs += ($p -split ';' | Where-Object { $_ }) }
    }
    return $dirs
}

# The path of a program on the PATH, or in the PATH that Windows has stored (a program installed a
# moment ago in this terminal); $null if not found.
function Find-Tool {
    param([string]$Name)
    $cmd = Get-Command $Name -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($cmd) { return $cmd.Source }
    foreach ($dir in Get-StoredPathDirs) {
        foreach ($ext in '.exe', '.cmd', '.bat') {
            $candidate = Join-Path $dir ($Name + $ext)
            if (Test-Path -LiteralPath $candidate) { return $candidate }
        }
    }
    return $null
}

function Confirm-Yes {
    param([string]$Question)
    if ($Yes) { return $true }
    $answer = & $script:Ask "$Question [y/N]"
    return ($answer.Trim() -match '^(y|yes|j|ja)$')
}

function Invoke-Build {
    # 1. tools
    $cmake = Find-Tool 'cmake'
    if (-not $cmake) {
        Write-Host 'CMake was not found. Run scripts\setup-dev-windows.ps1 first (it installs the tools).'
        return 2
    }
    $cargoDirs = @((Join-Path $env:USERPROFILE '.cargo\bin'))
    if (-not (Find-Tool 'cargo') -and -not (Test-Path -LiteralPath (Join-Path $cargoDirs[0] 'cargo.exe'))) {
        Write-Host 'cargo was not found. Run scripts\setup-dev-windows.ps1 first (it installs the tools).'
        return 2
    }
    $ninja = Find-Tool 'ninja'
    if ($ninja) { $env:Path = (Split-Path -Parent $ninja) + ';' + $env:Path }
    $env:Path = $cargoDirs[0] + ';' + $env:Path

    # 2. the build folder
    $full = Resolve-BuildFolder $BuildFolder $RepoRoot
    if (-not $full) {
        Write-Host "The build folder '$BuildFolder' must be inside the repository and not the repository itself."
        return 2
    }

    # 3. the generator
    $help = & $cmake --help 2>&1 | ForEach-Object { "$_" }
    $generators = @(Get-CMakeGenerators $help)
    $have = @{ ninja = [bool]$ninja; nmake = [bool](Find-Tool 'nmake'); make = [bool]((Find-Tool 'mingw32-make') -or (Find-Tool 'make')) }
    $choices = @(Get-GeneratorChoices $generators $have)
    if ($choices.Count -eq 0) {
        Write-Host 'CMake offers no build tool that is installed here. Install Ninja (scripts\setup-dev-windows.ps1) and run again.'
        return 2
    }
    $chosen = $null
    if ($Generator) {
        $chosen = $Generator
        if (@($generators | Where-Object { $_.Name -eq $chosen }).Count -eq 0) {
            Write-Host "CMake does not know the generator '$Generator'."
            return 2
        }
    } elseif ($Yes) {
        $chosen = $choices[0].Name
    } else {
        $n = Read-Choice 'Which build tool shall CMake use?' @($choices | ForEach-Object { $_.Text }) 1
        if ($null -eq $n) { Write-Host 'Cancelled.'; return 2 }
        $chosen = $choices[$n - 1].Name
    }

    # 4. the parts
    $keys = $null
    if ($Parts) {
        $keys = ConvertTo-PartList $Parts $script:AllParts
        if (-not $keys) { Write-Host "Parts must be 'all' or a list of: app, probe, check, test, dist."; return 2 }
    } elseif ($Yes) {
        $keys = @($script:AllParts | ForEach-Object { $_.Key })
    } else {
        $keys = Read-Parts $script:AllParts
        if (-not $keys) { Write-Host 'Cancelled.'; return 2 }
    }
    if (($keys -contains 'dist') -and ($keys -notcontains 'app')) {
        # dist needs the program; CMake builds it first (the target depends on it)
        Write-Host 'Note: dist builds usage-cockpit first.'
    }

    # 5. an existing folder of another tool
    $existing = Get-CachedGenerator $full
    if ($existing -and $existing -ne $chosen) {
        Write-Host ''
        Write-Host "The build folder $full was made with '$existing'. CMake cannot change that."
        if (-not (Test-Deletable $full $RepoRoot)) {
            Write-Host 'It is not a build folder of this repository that the script may delete. Choose another folder with -BuildFolder.'
            return 2
        }
        $delete = $ReplaceFolder -or (Confirm-Yes "Delete the folder and make it again with '$chosen'?")
        if (-not $delete) { Write-Host 'Cancelled; nothing was changed.'; return 2 }
        if (-not $PlanOnly) {
            Remove-Item -LiteralPath $full -Recurse -Force
            Write-Host "Deleted $full."
        } else {
            Write-Host "(plan only: the folder would be deleted)"
        }
    } elseif ((Test-Path -LiteralPath $full) -and -not $existing -and (@(Get-ChildItem -LiteralPath $full -Force -ErrorAction SilentlyContinue).Count -gt 0)) {
        Write-Host "The folder $full exists, is not empty and is not a CMake build folder. Choose another folder with -BuildFolder."
        return 2
    }

    # 6. the plan, the question, the run
    $steps = @(Get-Steps $keys $full $chosen $RepoRoot)
    Write-Host ''
    Write-Host "Build folder: $full"
    Write-Host "Build tool:   $chosen"
    Write-Host 'Plan:'
    Format-Plan $steps | ForEach-Object { Write-Host $_ }
    if ($PlanOnly) { return 0 }
    if (-not (Confirm-Yes 'Start?')) { Write-Host 'Cancelled; nothing was changed.'; return 2 }

    $results = @()
    $failed = $false
    foreach ($step in $steps) {
        Write-Host ''
        Write-Host "== $($step.Title)"
        if ($failed) { $results += [pscustomobject]@{ Title = $step.Title; Result = 'skipped' }; continue }
        Push-Location $RepoRoot
        try {
            $program = $step.Command[0]
            $arguments = @($step.Command | Select-Object -Skip 1)
            # Out-Host: the output is shown and is not part of the value the function returns.
            # Programs write progress to stderr; that is not an error of the script.
            $saved = $ErrorActionPreference
            $ErrorActionPreference = 'Continue'
            try { & $program @arguments 2>&1 | ForEach-Object { "$_" } | Out-Host } finally { $ErrorActionPreference = $saved }
            $code = $LASTEXITCODE
        } finally { Pop-Location }
        if ($code -eq 0) {
            $results += [pscustomobject]@{ Title = $step.Title; Result = 'ok' }
        } else {
            $results += [pscustomobject]@{ Title = $step.Title; Result = "FAILED (exit code $code)" }
            $failed = $true
        }
    }

    Write-Host ''
    Write-Host 'Result:'
    foreach ($r in $results) { Write-Host ("  {0,-34} {1}" -f $r.Title, $r.Result) }
    $exe = Join-Path $full 'cargo-target\release\usage-cockpit.exe'
    $probe = Join-Path $full 'cargo-target-probe\release\usage-probe.exe'
    $dist = Join-Path $full 'dist'
    if (Test-Path -LiteralPath $exe) { Write-Host "Program:        $exe" }
    if (Test-Path -LiteralPath $probe) { Write-Host "Test program:   $probe" }
    if (Test-Path -LiteralPath $dist) { Write-Host "Release files:  $dist" }
    if ($failed) { return 1 }
    return 0
}

if ($MyInvocation.InvocationName -ne '.') {
    exit (Invoke-Build)
}
