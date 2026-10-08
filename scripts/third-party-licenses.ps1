# SPDX-License-Identifier: Apache-2.0
<#
.SYNOPSIS
  Makes THIRD_PARTY_LICENSES.md (REQ-107) with cargo about, and with -Check also runs the licence
  check of cargo deny.

.DESCRIPTION
  Needs the tools: cargo install cargo-about cargo-deny --locked
  Without -Check the file in the repository root is written anew. With -Check nothing is
  written: cargo deny must accept every licence, and the committed file must be the one that
  would be written now (so that a new dependency cannot slip in unlisted).

.PARAMETER Check
  Only check; exit code 1 if a licence is not allowed or the file is out of date.
#>
param([switch]$Check)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo

function Need([string]$name) {
    if (-not (Get-Command $name -ErrorAction SilentlyContinue)) {
        throw "$name is not installed. Install the tools with: cargo install cargo-about cargo-deny --locked"
    }
}
Need 'cargo'

$target = Join-Path $repo 'THIRD_PARTY_LICENSES.md'
$fresh = [IO.Path]::GetTempFileName()
try {
    # cargo about must write to a file itself: redirecting its output in PowerShell breaks the encoding
    $log = & cargo about generate --workspace --output-file $fresh about.hbs 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "cargo about failed:`n$log" }
    # LF line ends, as in the rest of the repository
    $body = ([IO.File]::ReadAllText($fresh, [Text.Encoding]::UTF8) -replace "`r`n", "`n").TrimEnd() + "`n"
    [IO.File]::WriteAllText($fresh, $body, (New-Object Text.UTF8Encoding($false)))

    if ($Check) {
        & cargo deny check licenses
        if ($LASTEXITCODE -ne 0) { Write-Output 'A licence is not allowed (see above).'; exit 1 }
        $have = if (Test-Path $target) { [IO.File]::ReadAllText($target) -replace "`r`n", "`n" } else { '' }
        if ($have -ne $body) {
            Write-Output 'THIRD_PARTY_LICENSES.md is out of date. Run scripts/third-party-licenses.ps1 and commit the file.'
            exit 1
        }
        Write-Output 'Licences allowed and THIRD_PARTY_LICENSES.md is up to date.'
    } else {
        Copy-Item -LiteralPath $fresh -Destination $target -Force
        Write-Output "Wrote $target"
    }
} finally {
    Remove-Item -LiteralPath $fresh -Force -ErrorAction SilentlyContinue
}
