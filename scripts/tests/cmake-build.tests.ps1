# SPDX-License-Identifier: Apache-2.0
<#
.SYNOPSIS
  Tries the CMake build (CMakeLists.txt) on Windows: configure, build, dist, checksum file.

.DESCRIPTION
  Uses a temporary build folder (outside the repository), so nothing of the working tree or of
  `target\` is touched. The first run builds the whole program in release mode and takes a few
  minutes. With -RunTests it also runs `ctest` (cargo fmt, clippy and the tests of the workspace).

.PARAMETER RunTests
  Also run ctest.

.PARAMETER OtherTargets
  Also try the other targets from this computer: configure with -DCOCKPIT_TARGET for Linux x64,
  Linux arm64 and macOS arm64 and run the `check` target (cargo fmt --check and cargo clippy for that
  target). Nothing is linked or run, so this proves the configuration, the naming and that the code
  of that system passes the lints (clippy type-checks it); it does not build a program for it.

.PARAMETER Keep
  Do not delete the temporary build folder at the end.

.EXAMPLE
  powershell -File scripts\tests\cmake-build.tests.ps1
#>
[CmdletBinding()]
param(
    [switch]$RunTests,
    [switch]$OtherTargets,
    [switch]$Keep
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if (Test-Path -LiteralPath $cargoBin) { $env:PATH = "$cargoBin;$env:PATH" }

$failed = 0
function Assert-That {
    param([bool]$Condition, [string]$Name, [string]$Detail = '')
    if ($Condition) { Write-Host "ok    $Name" } else {
        Write-Host "FAIL  $Name"
        if ($Detail) { Write-Host "      $Detail" }
        $script:failed++
    }
}

function Invoke-Native {
    param([string]$Program, [string[]]$Arguments)
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $output = & $Program @Arguments 2>&1 | ForEach-Object { "$_" }
        return [pscustomobject]@{ Code = $LASTEXITCODE; Output = ($output -join "`n") }
    } finally { $ErrorActionPreference = $previous }
}

$cargoToml = Get-Content -LiteralPath (Join-Path $repo 'Cargo.toml') -Raw
$version = [regex]::Match($cargoToml, '(?m)^version = "([^"]+)"').Groups[1].Value
Assert-That ([bool]$version) 'the workspace version is read from Cargo.toml' $version

$targetFolder = Join-Path $repo 'target'
$targetBefore = if (Test-Path -LiteralPath $targetFolder) { (Get-ChildItem -LiteralPath $targetFolder -Recurse -Force -File -ErrorAction SilentlyContinue | Measure-Object).Count } else { 0 }

$build = Join-Path ([IO.Path]::GetTempPath()) ('cockpit-cmake-test-' + [guid]::NewGuid().ToString('N'))
try {
    # configure
    $r = Invoke-Native 'cmake' @('-S', $repo, '-B', $build)
    Assert-That ($r.Code -eq 0) 'configure works' $r.Output
    Assert-That ($r.Output -match [regex]::Escape("usage-cockpit $version, target x86_64-pc-windows-msvc (windows-x64)")) 'configure names the version and the system (Windows x64)'

    # an in-source build is refused (tried on a copy of the files: CMake writes its cache before the
    # check runs, which must not happen in the working tree)
    $copy = Join-Path ([IO.Path]::GetTempPath()) ('cockpit-cmake-test-src-' + [guid]::NewGuid().ToString('N'))
    try {
        [void](New-Item -ItemType Directory (Join-Path $copy 'cmake'))
        Copy-Item -LiteralPath (Join-Path $repo 'CMakeLists.txt'), (Join-Path $repo 'Cargo.toml') -Destination $copy
        Copy-Item -LiteralPath (Join-Path $repo 'cmake\dist.cmake') -Destination (Join-Path $copy 'cmake')
        $inSource = Invoke-Native 'cmake' @('-S', $copy, '-B', $copy)
        Assert-That ($inSource.Code -ne 0 -and $inSource.Output -match 'build folder of its own') 'an in-source build is refused' $inSource.Output
    } finally { Remove-Item -LiteralPath $copy -Recurse -Force -ErrorAction SilentlyContinue }

    # an unsupported target is refused with a clear message
    $other = Join-Path ([IO.Path]::GetTempPath()) ('cockpit-cmake-test-bad-' + [guid]::NewGuid().ToString('N'))
    try {
        $bad = Invoke-Native 'cmake' @('-S', $repo, '-B', $other, '-DCOCKPIT_TARGET=riscv64-unknown-none')
        Assert-That ($bad.Code -ne 0 -and $bad.Output -match 'not one of the supported targets') 'an unsupported target is refused' $bad.Output
    } finally { Remove-Item -LiteralPath $other -Recurse -Force -ErrorAction SilentlyContinue }

    # the tests are registered
    $list = Invoke-Native 'ctest' @('--test-dir', $build, '-N')
    Assert-That ($list.Output -match 'cargo-fmt' -and $list.Output -match 'cargo-clippy' -and $list.Output -match 'cargo-test') 'ctest knows fmt, clippy and the tests' $list.Output

    # dist
    $d = Invoke-Native 'cmake' @('--build', $build, '--config', 'Release', '--target', 'dist')
    Assert-That ($d.Code -eq 0) 'the dist target builds' ($d.Output -split "`n" | Select-Object -Last 15 | Out-String)
    $name = "usage-cockpit-$version-windows-x64.exe"
    $exe = Join-Path $build "dist\$name"
    Assert-That (Test-Path -LiteralPath $exe) "dist holds $name"

    if (Test-Path -LiteralPath $exe) {
        $sums = Join-Path $build 'dist\SHA256SUMS'
        $hash = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLower()
        $line = (Get-Content -LiteralPath $sums | Where-Object { $_ -like "*  $name" })
        Assert-That ($line -ceq "$hash  $name") 'SHA256SUMS has the checksum of the file' "$line"

        $v = Invoke-Native $exe @('--version')
        Assert-That ($v.Code -eq 0 -and $v.Output -match "usage-cockpit $([regex]::Escape($version)) \(") 'the program in dist starts and prints its version' $v.Output

        # a second run keeps one line for the file and the lines of other systems
        Add-Content -LiteralPath $sums -Value ('0' * 64 + '  usage-cockpit-' + $version + '-linux-x64')
        $again = Invoke-Native 'cmake' @('--build', $build, '--config', 'Release', '--target', 'dist')
        Assert-That ($again.Code -eq 0) 'dist can be run again'
        $lines = @(Get-Content -LiteralPath $sums | Where-Object { $_ })
        Assert-That ($lines.Count -eq 2) 'the sums file has one line per file' ($lines -join ' | ')
        Assert-That (@($lines | Where-Object { $_ -like "*linux-x64" }).Count -eq 1) 'the line of another system is kept'
        Assert-That ($lines[0] -like '*linux-x64' -and $lines[1] -like "*windows-x64.exe") 'the lines are sorted by file name'
    }

    # cargo output stays in the build folder, and the folder `target` of plain cargo runs is untouched
    $targetAfter = if (Test-Path -LiteralPath $targetFolder) { (Get-ChildItem -LiteralPath $targetFolder -Recurse -Force -File -ErrorAction SilentlyContinue | Measure-Object).Count } else { 0 }
    Assert-That ($targetAfter -eq $targetBefore) 'the target folder of plain cargo runs was not touched' "before $targetBefore, after $targetAfter files"
    Assert-That (Test-Path -LiteralPath (Join-Path $build 'cargo-target\release\usage-cockpit.exe')) 'cargo built into the build folder'

    if ($OtherTargets) {
        $others = @(
            @{ Triple = 'x86_64-unknown-linux-gnu'; System = 'linux-x64' },
            @{ Triple = 'aarch64-unknown-linux-gnu'; System = 'linux-arm64' },
            @{ Triple = 'aarch64-apple-darwin'; System = 'macos-arm64' }
        )
        foreach ($other in $others) {
            $folder = Join-Path ([IO.Path]::GetTempPath()) ('cockpit-cmake-test-' + $other.System + '-' + [guid]::NewGuid().ToString('N'))
            try {
                $c = Invoke-Native 'cmake' @('-S', $repo, '-B', $folder, "-DCOCKPIT_TARGET=$($other.Triple)")
                Assert-That ($c.Code -eq 0) "$($other.System): configure works" $c.Output
                Assert-That ($c.Output -match [regex]::Escape("target $($other.Triple) ($($other.System))")) "$($other.System): the system name follows from the target"
                Assert-That ($c.Output -match 'only compiles the tests') "$($other.System): the tests are only compiled for a target that is not the host"
                $k = Invoke-Native 'cmake' @('--build', $folder, '--config', 'Release', '--target', 'check')
                Assert-That ($k.Code -eq 0) "$($other.System): the check target (fmt, clippy for that target) passes" ($k.Output -split "`n" | Select-Object -Last 12 | Out-String)
            } finally { Remove-Item -LiteralPath $folder -Recurse -Force -ErrorAction SilentlyContinue }
        }
    }

    if ($RunTests) {
        $t = Invoke-Native 'ctest' @('--test-dir', $build, '-C', 'Release', '--output-on-failure')
        Assert-That ($t.Code -eq 0) 'ctest passes (fmt, clippy, tests)' ($t.Output -split "`n" | Select-Object -Last 25 | Out-String)
    }
} finally {
    if (-not $Keep) { Remove-Item -LiteralPath $build -Recurse -Force -ErrorAction SilentlyContinue }
}

if ($failed -gt 0) { Write-Host "$failed check(s) failed"; exit 1 }
Write-Host 'all checks passed'
