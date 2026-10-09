# SPDX-License-Identifier: Apache-2.0
<#
.SYNOPSIS
  Tests of scripts\build.ps1: the pure parts, the questions (answers are injected) and the plan.
  Nothing is built and no folder of the repository is changed.

.EXAMPLE
  powershell -File scripts\tests\build.tests.ps1
#>
$ErrorActionPreference = 'Stop'
. (Join-Path (Split-Path -Parent $PSScriptRoot) 'build.ps1')

$failed = 0
function Assert-Equal {
    param($Actual, $Expected, [string]$Name)
    if (($Actual -join '|') -ceq ($Expected -join '|')) {
        Write-Host "ok    $Name"
    } else {
        Write-Host "FAIL  $Name"
        Write-Host "      expected: $($Expected -join '|')"
        Write-Host "      actual:   $($Actual -join '|')"
        $script:failed++
    }
}
function Assert-True {
    param($Condition, [string]$Name, [string]$Detail = '')
    if ($Condition) { Write-Host "ok    $Name" } else { Write-Host "FAIL  $Name"; if ($Detail) { Write-Host "      $Detail" }; $script:failed++ }
}

# ---- the generators in the help text of CMake
$help = @(
    'Options',
    '  -S <path-to-source>          = Explicitly specify a source directory.',
    '  -G <generator-name>          = Specify a build system generator.',
    '',
    'Generators',
    '',
    'The following generators are available on this platform (* marks default):',
    '* Visual Studio 17 2022        = Generates Visual Studio 2022 project files.',
    '                                 Use -A option to specify architecture.',
    '  Visual Studio 16 2019        = Generates Visual Studio 2019 project files.',
    '                                 Use -A option to specify architecture.',
    '  Ninja                        = Generates build.ninja files.',
    '  Ninja Multi-Config           = Generates build-<Config>.ninja files.',
    '  NMake Makefiles              = Generates NMake makefiles.',
    '  MinGW Makefiles              = Generates a make file for use with',
    '                                 mingw32-make.',
    '  Unix Makefiles               = Generates standard UNIX makefiles.'
)
$gens = @(Get-CMakeGenerators $help)
Assert-Equal ($gens | ForEach-Object { $_.Name }) @('Visual Studio 17 2022', 'Visual Studio 16 2019', 'Ninja', 'Ninja Multi-Config', 'NMake Makefiles', 'MinGW Makefiles', 'Unix Makefiles') 'the generators are read from the help text'
Assert-Equal ($gens | Where-Object { $_.IsDefault } | ForEach-Object { $_.Name }) @('Visual Studio 17 2022') 'the default generator is the one with the star'
Assert-Equal @(Get-CMakeGenerators @()).Count 0 'no help text gives no generators'
Assert-Equal @(Get-CMakeGenerators @('  Ninja = not inside the section')).Count 0 'lines before the section Generators are not read'

# ---- the choices
$choices = @(Get-GeneratorChoices $gens @{ ninja = $true; nmake = $false; make = $false })
Assert-Equal ($choices | ForEach-Object { $_.Name }) @('Ninja', 'Visual Studio 17 2022') 'with Ninja: Ninja first, then the default of CMake'
Assert-Equal ([bool]$choices[0].Recommended) $true 'Ninja is recommended'
$choices = @(Get-GeneratorChoices $gens @{ ninja = $false; nmake = $false; make = $false })
Assert-Equal ($choices | ForEach-Object { $_.Name }) @('Visual Studio 17 2022') 'without Ninja: only the default of CMake'
$choices = @(Get-GeneratorChoices $gens @{ ninja = $true; nmake = $true; make = $true })
Assert-Equal ($choices | ForEach-Object { $_.Name }) @('Ninja', 'Visual Studio 17 2022', 'NMake Makefiles', 'MinGW Makefiles', 'Unix Makefiles') 'all tools found: all offered, no double entries'
$onlyNinjaDefault = @(Get-CMakeGenerators @('Generators', '* Ninja = x', '  Unix Makefiles = y'))
Assert-Equal (@(Get-GeneratorChoices $onlyNinjaDefault @{ ninja = $true; nmake = $false; make = $false }) | ForEach-Object { $_.Name }) @('Ninja') 'Ninja as default of CMake is offered once'

# ---- the questions (answers injected)
function Set-Answers {
    param([string[]]$Answers)
    $script:queue = [System.Collections.Queue]::new()
    foreach ($a in $Answers) { $script:queue.Enqueue($a) }
    $script:Ask = { param([string]$Prompt) if ($script:queue.Count -eq 0) { throw "asked more than expected: $Prompt" } else { [string]$script:queue.Dequeue() } }
}
$options = @('first', 'second', 'third')
Set-Answers @('')
Assert-Equal (Read-Choice 'T' $options 2) 2 'an empty answer is the default'
Set-Answers @('3')
Assert-Equal (Read-Choice 'T' $options 1) 3 'a number is taken'
Set-Answers @('x', '9', '1')
Assert-Equal (Read-Choice 'T' $options 2) 1 'wrong answers are asked again'
Set-Answers @('q')
Assert-Equal ([string](Read-Choice 'T' $options 1)) '' 'q cancels'
Set-Answers @('a', 'b', 'c')
Assert-Equal ([string](Read-Choice 'T' $options 1)) '' 'three wrong answers cancel'

# ---- the parts
$keys = @($script:AllParts | ForEach-Object { $_.Key })
Assert-Equal $keys @('app', 'probe', 'check', 'test', 'dist') 'the parts in the order of the run'
Assert-Equal (ConvertTo-PartList 'all' $script:AllParts) $keys '"all" is every part'
Assert-Equal (ConvertTo-PartList 'dist, APP' $script:AllParts) @('app', 'dist') 'a list is taken in the order of the parts, case and spaces do not matter'
Assert-Equal ([string](ConvertTo-PartList 'app,foo' $script:AllParts)) '' 'an unknown part is refused'
Assert-Equal ([string](ConvertTo-PartList '' $script:AllParts)) '' 'an empty text is refused'
Set-Answers @('1')
Assert-Equal (Read-Parts $script:AllParts) $keys 'everything is the first choice'
Set-Answers @('2', '1,3')
Assert-Equal (Read-Parts $script:AllParts) @('app', 'check') 'a choice by numbers'
Set-Answers @('2', '9', '4,5')
Assert-Equal (Read-Parts $script:AllParts) @('test', 'dist') 'a wrong number list is asked again'
Set-Answers @('2', 'q')
Assert-Equal ([string](Read-Parts $script:AllParts)) '' 'q cancels the choice of the parts'

# ---- folders
$temp = Join-Path ([IO.Path]::GetTempPath()) ('cockpit-build-test-' + [guid]::NewGuid().ToString('N'))
$repo = Join-Path $temp 'repo'
[void](New-Item -ItemType Directory $repo)
try {
    Assert-Equal (Resolve-BuildFolder 'build' $repo) (Join-Path $repo 'build') 'a relative folder is made absolute inside the repository'
    Assert-Equal (Resolve-BuildFolder (Join-Path $repo 'out\x') $repo) (Join-Path $repo 'out\x') 'an absolute folder inside the repository is kept'
    Assert-Equal ([string](Resolve-BuildFolder $temp $repo)) '' 'a folder above the repository is refused'
    Assert-Equal ([string](Resolve-BuildFolder '..\other' $repo)) '' 'a folder that leaves the repository by .. is refused'
    Assert-Equal ([string](Resolve-BuildFolder '.' $repo)) '' 'the repository itself is refused'
    Assert-Equal ([string](Resolve-BuildFolder '' $repo)) '' 'an empty name is refused'
    Assert-Equal ([string](Resolve-BuildFolder ($repo + '-evil\build') $repo)) '' 'a folder next to the repository with the same start of the name is refused'

    $b = Join-Path $repo 'build'
    [void](New-Item -ItemType Directory $b)
    Assert-Equal ([string](Get-CachedGenerator $b)) '' 'a folder without a cache has no generator'
    Assert-Equal (Test-Deletable $b $repo) $false 'a folder without a cache is not deleted'
    $homeLine = 'CMAKE_HOME_DIRECTORY:INTERNAL=' + $repo.Replace('\', '/')
    Set-Content -LiteralPath (Join-Path $b 'CMakeCache.txt') -Value @('# cache', 'CMAKE_GENERATOR:INTERNAL=Visual Studio 17 2022', $homeLine)
    Assert-Equal (Get-CachedGenerator $b) 'Visual Studio 17 2022' 'the generator is read from the cache'
    Assert-Equal (Test-Deletable $b $repo) $true 'a build folder of this repository may be deleted'
    Set-Content -LiteralPath (Join-Path $b 'CMakeCache.txt') -Value @('CMAKE_GENERATOR:INTERNAL=Ninja', 'CMAKE_HOME_DIRECTORY:INTERNAL=C:/somewhere/else')
    Assert-Equal (Test-Deletable $b $repo) $false 'a build folder of another source folder is not deleted'
    Assert-Equal (Test-Deletable '' $repo) $false 'no folder is not deleted'
} finally {
    Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
}

# ---- the steps
$steps = @(Get-Steps $keys 'C:\r\build' 'Ninja' 'C:\r')
Assert-Equal ($steps | ForEach-Object { $_.Title }) @('configure with Ninja', 'build usage-cockpit', 'build tools/usage-probe', 'checks (fmt and clippy)', 'tests (ctest)', 'release file (dist)') 'everything: six steps in order'
Assert-Equal ($steps[0].Command) @('cmake', '-S', 'C:\r', '-B', 'C:\r\build', '-G', 'Ninja') 'configure names source, folder and generator'
Assert-Equal ($steps[2].Command) @('cmake', '--build', 'C:\r\build', '--config', 'Release', '--target', 'usage-probe') 'the probe has its own target'
Assert-Equal ($steps[4].Command) @('ctest', '--test-dir', 'C:\r\build', '-C', 'Release', '--output-on-failure') 'the tests run through ctest'
$few = @(Get-Steps @('probe') 'C:\r\build' 'Ninja' 'C:\r')
Assert-Equal ($few | ForEach-Object { $_.Title }) @('configure with Ninja', 'build tools/usage-probe') 'a choice gives configure and the chosen parts only'
Assert-Equal @(Format-Plan $few).Count 2 'the plan has one entry per step'
Assert-True ((Format-Plan $few)[0] -match 'configure with Ninja') 'the plan names the step'

# ---- the whole script, with the plan only (nothing is run, no folder is made)
$script1 = Join-Path (Split-Path -Parent $PSScriptRoot) 'build.ps1'
function Invoke-Script {
    param([string[]]$Arguments, [string]$Input1 = '')
    $out = if ($Input1) { $Input1 | & powershell -NoProfile -ExecutionPolicy Bypass -File $script1 @Arguments 2>&1 } else { & powershell -NoProfile -ExecutionPolicy Bypass -File $script1 @Arguments 2>&1 }
    return [pscustomobject]@{ Code = $LASTEXITCODE; Text = ($out | ForEach-Object { "$_" }) -join "`n" }
}
$folder = 'build-plan-test-' + [guid]::NewGuid().ToString('N').Substring(0, 8)
$r = Invoke-Script @('-PlanOnly', '-Yes', '-Generator', 'Ninja', '-Parts', 'app,probe', '-BuildFolder', $folder)
if ($r.Text -match 'CMake was not found|cargo was not found') {
    Write-Host 'skip  the run of the whole script (CMake or cargo is not installed here)'
} else {
    Assert-Equal $r.Code 0 'plan only: exit code 0'
    Assert-True ($r.Text -match 'build usage-cockpit' -and $r.Text -match 'build tools/usage-probe' -and $r.Text -notmatch 'dist') 'plan only: the chosen parts are in the plan' $r.Text
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $RepoRoot $folder))) 'plan only: no folder was made'
    $r = Invoke-Script @('-PlanOnly', '-Yes', '-Parts', 'nonsense', '-BuildFolder', $folder)
    Assert-Equal $r.Code 2 'wrong parts: exit code 2'
    $r = Invoke-Script @('-PlanOnly', '-Yes', '-Generator', 'No Such Generator', '-BuildFolder', $folder)
    Assert-Equal $r.Code 2 'an unknown generator: exit code 2'
    $r = Invoke-Script @('-PlanOnly', '-Yes', '-BuildFolder', '..\outside')
    Assert-Equal $r.Code 2 'a folder outside the repository: exit code 2'
    $r = Invoke-Script @('-PlanOnly', '-BuildFolder', $folder) "1`n1`n"
    Assert-Equal $r.Code 0 'asked: the first tool and everything give a plan'
    Assert-True ($r.Text -match 'Which build tool shall CMake use\?' -and $r.Text -match 'What shall be built\?' -and $r.Text -match 'release file \(dist\)') 'asked: both questions are shown and everything is planned' $r.Text
    $r = Invoke-Script @('-PlanOnly', '-BuildFolder', $folder) "1`nq`n"
    Assert-Equal $r.Code 2 'asked: q at the second question cancels'

    # a folder made with another tool: -Yes alone does not delete it, -ReplaceFolder would (plan only here)
    $other = Join-Path $RepoRoot ('build-other-test-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
    [void](New-Item -ItemType Directory $other)
    try {
        $rootText = $RepoRoot.Replace([string][char]92, '/')
        Set-Content -LiteralPath (Join-Path $other 'CMakeCache.txt') -Value @('CMAKE_GENERATOR:INTERNAL=Unix Makefiles', "CMAKE_HOME_DIRECTORY:INTERNAL=$rootText")
        $name = Split-Path -Leaf $other
        $r = Invoke-Script @('-PlanOnly', '-Yes', '-Generator', 'Ninja', '-BuildFolder', $name)
        Assert-Equal $r.Code 2 'another tool, -Yes alone: nothing is deleted, exit code 2'
        Assert-True ($r.Text -match 'ReplaceFolder') '-Yes alone: the message names -ReplaceFolder' $r.Text
        Assert-True (Test-Path -LiteralPath (Join-Path $other 'CMakeCache.txt')) '-Yes alone: the folder is still there'
        $r = Invoke-Script @('-PlanOnly', '-Yes', '-ReplaceFolder', '-Generator', 'Ninja', '-BuildFolder', $name)
        Assert-Equal $r.Code 0 '-ReplaceFolder with a plan only: exit code 0'
        Assert-True (Test-Path -LiteralPath (Join-Path $other 'CMakeCache.txt')) 'plan only: even with -ReplaceFolder nothing is deleted'
        $r = Invoke-Script @('-PlanOnly', '-Generator', 'Ninja', '-Parts', 'app', '-BuildFolder', $name) "n`n"
        Assert-Equal $r.Code 2 'another tool, answer no: cancelled, exit code 2'
        Assert-True (Test-Path -LiteralPath (Join-Path $other 'CMakeCache.txt')) 'answer no: the folder is still there'
    } finally {
        Remove-Item -LiteralPath $other -Recurse -Force -ErrorAction SilentlyContinue
    }
}

if ($failed -gt 0) { Write-Host "`n$failed check(s) failed."; exit 1 }
Write-Host "`nall checks passed"
