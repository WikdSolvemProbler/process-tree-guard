# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
# Build everything the suite needs, end to end, on a Windows laptop.
#
#   .\build.ps1 [-Tree <path-to-guard-tree>] [-Competitors <path>]
#
# Steps: locate MSVC (vswhere), build the guard's four role programs
# and its test helper with the MSVC target in release, verify the
# binaries, and stage them into build\ beside this script. Competitor
# binaries are package inputs, supplied as found: the staging source
# is the folder -Competitors names, or, when it names none, the
# preserved bundle beside this package (see the staging-source block
# below). Either way the binaries from that source, holding
# processkit-cli.exe, timeout.exe (nuewframe) and sysprims.exe,
# are verified against the pinned identities in campaign.json
# (file name, probed version, and the SHA-256 pin — the anchor;
# a hash mismatch throws naming the arm, the expected pin, and
# the actual hash), copied, untouched,
# into build\competitors\, and their SHA-256 hashes and probed
# versions are recorded into build\competitor-identities.json, an
# added consistency record later campaign runs are also checked
# against (the pin in campaign.json is the anchor, not that
# record). The build
# never fetches or patches them.
#
# Every build also writes the staged build record,
# build\build-record.json: the tree this build built, recorded as
# tree_as_given (the -Tree value as given on the command line) and
# tree_resolved (its resolved absolute form). The record is written
# on every build, whether or not competitors were staged, and
# run.ps1 checks it before running: a run whose tree differs from
# the recorded tree is refused, naming both trees, before any
# stage runs.
#
# The cargo build runs under a finite deadline ($BuildDeadlineSeconds,
# 1,800 s): a build that has not finished by then has its whole
# process tree terminated and the script fails, naming the deadline.
#
# Tier: Windows-theory from the Linux development machine (authored
# to be correct by construction; its syntax and staging logic are
# exercised there as far as the platform allows — see VERIFICATION.md).

param(
    [string]$Tree = (Join-Path $PSScriptRoot ".."),
    [string]$Competitors = ""
)
$ErrorActionPreference = "Stop"

# --- competitor staging source -------------------------------------------
# With no -Competitors folder named, the staging source defaults to
# the preserved competitor bundle beside this package, at the first
# of these locations that exists: the bundle inside the guard tree
# checkout beside the suite, the bundle at the root of the repo the
# suite sits in, or the bundle inside this folder. An explicitly
# named -Competitors folder is the override and always wins. The
# SHA-256 pins in campaign.json are verified either way; the default
# changes where the binaries come from, never what is accepted.
if ($Competitors -eq "") {
    $bundleCandidates = @(
        (Join-Path $PSScriptRoot "..\tree\benchmark\competitors-001\bins"),
        (Join-Path $PSScriptRoot "..\benchmark\competitors-001\bins"),
        (Join-Path $PSScriptRoot "benchmark\competitors-001\bins")
    )
    foreach ($candidate in $bundleCandidates) {
        if (Test-Path $candidate) {
            $Competitors = $candidate
            break
        }
    }
    if ($Competitors -ne "") {
        Write-Host "competitor staging source: the preserved bundle at $Competitors"
    } else {
        Write-Host "competitor staging source: none found beside the package and no -Competitors folder named; the guard alone will be staged"
    }
}

# --- prerequisites -------------------------------------------------------
# Preflight, before any stage runs: cargo and the MSVC target must
# be present, or the build dies later with a raw native error and
# no stage record. Each failure names the missing prerequisite
# and its remedy.
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw "missing prerequisite: cargo not found on PATH; remedy: install the Rust toolchain from https://rustup.rs and ensure cargo is on PATH" }
if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) { throw "missing prerequisite: rustup not found on PATH; remedy: install the Rust toolchain from https://rustup.rs so the x86_64-pc-windows-msvc target can be checked" }
$installedTargets = & rustup target list --installed
if ($installedTargets -notcontains "x86_64-pc-windows-msvc") { throw "missing prerequisite: Rust target x86_64-pc-windows-msvc not installed; remedy: run 'rustup target add x86_64-pc-windows-msvc'" }

# --- locate MSVC -------------------------------------------------------
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path $vswhere)) { throw "vswhere.exe not found; install Visual Studio Build Tools with the C++ workload" }
$install = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $install) { throw "no MSVC C++ toolset found by vswhere" }
$vcvars = Join-Path $install "VC\Auxiliary\Build\vcvars64.bat"
if (-not (Test-Path $vcvars)) { throw "vcvars64.bat not found under $install" }

# --- build the guard (MSVC target, release) -----------------------------
# Finite build deadline: the workspace convention caps a build or test
# command at 1,800 s. The cargo invocation below runs as a child
# process waited on with exactly that bound; on expiry its process
# tree is killed and the script fails. A build inside the bound is
# waited on normally and its exit code is honoured unchanged.
$BuildDeadlineSeconds = 1800
$target = "x86_64-pc-windows-msvc"
$env:CARGO_TARGET_DIR = Join-Path $PSScriptRoot "build\cargo-target"
Push-Location $Tree
try {
    $cmd = "`"$vcvars`" && cargo build --release --target $target --all-targets"
    $proc = Start-Process -FilePath "cmd.exe" -ArgumentList "/c", $cmd `
        -NoNewWindow -PassThru
    # Windows PowerShell 5.1 reports no exit code for a process whose
    # handle was never opened; reading Handle here keeps ExitCode valid.
    $null = $proc.Handle
    if (-not $proc.WaitForExit($BuildDeadlineSeconds * 1000)) {
        & taskkill.exe /PID $proc.Id /T /F | Out-Null
        $proc.WaitForExit()
        throw "cargo build exceeded its finite deadline of $BuildDeadlineSeconds s; its process tree was terminated"
    }
    if ($proc.ExitCode -ne 0) { throw "cargo build failed with $($proc.ExitCode)" }
} finally { Pop-Location }

# --- stage ---------------------------------------------------------------
$stage = Join-Path $PSScriptRoot "build"
New-Item -ItemType Directory -Force $stage | Out-Null
$built = Join-Path $env:CARGO_TARGET_DIR "$target\release"
foreach ($name in @("process-tree-guard.exe", "process-tree-guard-deadline.exe",
                    "process-tree-guard-gate.exe", "process-tree-guard-contain.exe",
                    "testkit.exe")) {
    $src = Join-Path $built $name
    if (-not (Test-Path $src)) {
        # A bin gated on a feature the dev-dependency enables (testkit)
        # may land in deps\ under --all-targets.
        $alt = Get-ChildItem (Join-Path $built "deps") -Filter $name -ErrorAction SilentlyContinue |
               Select-Object -First 1
        if ($alt) { $src = $alt.FullName }
    }
    if (-not (Test-Path $src)) { throw "expected build product missing: $src" }
    Copy-Item $src (Join-Path $stage $name) -Force
}
# ptg.exe is the short name of the full program: a copy of the same image; no separate build is needed.
Copy-Item (Join-Path $stage "process-tree-guard.exe") (Join-Path $stage "ptg.exe") -Force

if ($Competitors -ne "") {
    $dest = Join-Path $stage "competitors"
    New-Item -ItemType Directory -Force $dest | Out-Null
    # Pin check: the expected competitor identities live in
    # campaign.json (competitor_identities) and the SHA-256 pin
    # there is the anchor. Each supplied binary's SHA-256 is
    # verified against its pin BEFORE it is staged and before the
    # staging record is written; a mismatch throws naming the arm,
    # the expected pin, and the actual hash. Each binary is also
    # probed for its version before it is staged; a binary whose
    # probed version disagrees with the pinned version fails the
    # build with the mismatch named, so a same-named wrong binary
    # (the guard renamed to timeout.exe, say) is caught here, never
    # measured as that competitor. Staged hashes and probed versions
    # are recorded into build\competitor-identities.json as an
    # added consistency record — the pin is the anchor, not it.
    $configPath = Join-Path $PSScriptRoot "campaign.json"
    $config = Get-Content $configPath -Raw | ConvertFrom-Json
    $record = [ordered]@{}
    foreach ($arm in @("processkit", "nuewframe", "sysprims")) {
        $spec = $config.competitor_identities.$arm
        $name = $spec.binary
        $src = Join-Path $Competitors $name
        if (-not (Test-Path $src)) { throw "competitor input missing (supplied as found, never fetched): $src" }
        $expectedPin = $spec.sha256
        if (-not $expectedPin) { throw "competitor identity mismatch: $arm ($name) has no pinned sha256 in campaign.json; refusing to stage" }
        $actualHash = (Get-FileHash $src -Algorithm SHA256).Hash.ToLower()
        if ($actualHash -ne $expectedPin) {
            throw "competitor identity mismatch: $arm ($name) sha256 actual '$actualHash' != expected pin '$expectedPin' in campaign.json; refusing to stage"
        }
        $versionArgs = @($spec.version_args)
        # The version probe merges the native stderr stream into
        # the captured output. Under the Stop preference set at
        # the top of this script, a native command whose stderr
        # is redirected is surfaced as a terminating error in
        # Windows PowerShell, so any stderr noise from the probed
        # binary would abort the staging before the version could
        # be read. The probe therefore runs under Continue and
        # the script preference is restored immediately after;
        # the probed version check below is unchanged.
        $ErrorActionPreference = "Continue"
        $probeOutput = (& $src @versionArgs 2>&1 | Out-String)
        $ErrorActionPreference = "Stop"
        $probed = $null
        if ($probeOutput -match "\d+\.\d+\.\d+") { $probed = $Matches[0] }
        if ($probed -ne $spec.version) {
            throw "competitor identity mismatch: $arm ($name) probed version '$probed' != pinned version '$($spec.version)' in campaign.json; refusing to stage"
        }
        Copy-Item $src (Join-Path $dest $name) -Force
        $hash = (Get-FileHash (Join-Path $dest $name) -Algorithm SHA256).Hash.ToLower()
        if ($hash -ne $expectedPin) {
            throw "competitor identity mismatch: $arm ($name) staged sha256 actual '$hash' != expected pin '$expectedPin' in campaign.json; refusing to stage"
        }
        $record[$arm] = [ordered]@{ binary = $name; version = $probed; sha256 = $hash }
        Write-Host "staged competitor $arm ($name) version $probed sha256 $hash"
    }
    $record | ConvertTo-Json | Set-Content (Join-Path $stage "competitor-identities.json") -Encoding UTF8
}

# --- build record ----------------------------------------------------------
# The staged build record: which tree this build built, as given
# on the command line and in resolved absolute form. Written on
# every build, whether or not competitors were staged, so a later
# run.ps1 can refuse to audit a tree this build did not produce.
$treeResolved = (Resolve-Path -LiteralPath $Tree).Path
$buildRecord = [ordered]@{ tree_as_given = $Tree; tree_resolved = $treeResolved }
$buildRecord | ConvertTo-Json | Set-Content (Join-Path $stage "build-record.json") -Encoding UTF8

Write-Host "built and staged into $stage"
