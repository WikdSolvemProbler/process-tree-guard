# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
#
# Install process-tree-guard for the current user on Windows.
#
#   irm https://raw.githubusercontent.com/WikdSolvemProbler/process-tree-guard/main/install.ps1 | iex
#
# What it does, in order, and nothing is installed when any check fails:
# 1. Reads the machine: 64-bit Windows on x86-64 runs the programs natively; Windows on Arm64
#    runs them through its built-in x64 emulation; 32-bit Windows is refused.
# 2. Downloads the latest release zip (or the version in $env:PTG_VERSION, for example 0.5.0)
#    and checks the zip and every file in it against the release's SHA256SUMS.
# 3. Installs the documented install layout under %LOCALAPPDATA%\Programs\process-tree-guard:
#    bin\ holds the full program and its automatic copy, libexec\ holds the roles and the
#    deadline role's automatic copy, and POLICY.md stands at the root, one folder above both.
#    Short names (ptg.exe and ptg-auto.exe) are installed beside the full program when the
#    release carries ptg.exe.
# 4. Adds bin\ to the user's PATH (skipped when $env:PTG_NO_PATH is set; $env:PTG_ROOT moves the install).
# 5. Finds the coding agents on this machine (Claude Code, Cursor, Codex, Antigravity) and
#    writes each one's hook configuration, with this machine's paths, to hooks\ under the
#    install folder. It does not edit any agent's own settings; it prints where to merge each file.

$ErrorActionPreference = "Stop"

# JSON without a byte order mark, which some agent hosts do not accept.
function Write-Json($value, $path) {
    [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 8), (New-Object Text.UTF8Encoding $false))
}
$repo = "WikdSolvemProbler/process-tree-guard"
$target = "x86_64-pc-windows-msvc"
$root = if ($env:PTG_ROOT) { $env:PTG_ROOT } else { Join-Path $env:LOCALAPPDATA "Programs\process-tree-guard" }
$bin = Join-Path $root "bin"
$libexec = Join-Path $root "libexec"

# 1. The machine.
if (-not [Environment]::Is64BitOperatingSystem) { throw "process-tree-guard needs 64-bit Windows." }
$arch = $env:PROCESSOR_ARCHITEW6432
if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
switch ($arch) {
    "AMD64" { Write-Host "Machine: Windows x86-64; the programs run natively." }
    "ARM64" { Write-Host "Machine: Windows on Arm64; the x86-64 programs run through Windows x64 emulation." }
    default { throw "Unsupported processor architecture: $arch." }
}

# 2. The release, checked.
$headers = @{ "User-Agent" = "process-tree-guard-installer" }
if ($env:PTG_VERSION) {
    $release = Invoke-RestMethod -Headers $headers "https://api.github.com/repos/$repo/releases/tags/v$($env:PTG_VERSION)"
} else {
    $release = Invoke-RestMethod -Headers $headers "https://api.github.com/repos/$repo/releases/latest"
}
$version = $release.tag_name.TrimStart("v")
$zipName = "process-tree-guard-$version-$target.zip"
$zipAsset = $release.assets | Where-Object { $_.name -eq $zipName }
$sumAsset = $release.assets | Where-Object { $_.name -eq "SHA256SUMS" }
if (-not $zipAsset -or -not $sumAsset) { throw "Release v$version has no $zipName with SHA256SUMS." }

$work = Join-Path ([IO.Path]::GetTempPath()) ("ptg-install-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force $work | Out-Null
try {
    Write-Host "Downloading process-tree-guard $version"
    $zip = Join-Path $work $zipName
    $sums = Join-Path $work "SHA256SUMS"
    Invoke-WebRequest -Headers $headers -Uri $zipAsset.browser_download_url -OutFile $zip -UseBasicParsing
    Invoke-WebRequest -Headers $headers -Uri $sumAsset.browser_download_url -OutFile $sums -UseBasicParsing

    $expected = @{}
    foreach ($line in Get-Content $sums) {
        if ($line -match '^([0-9a-f]{64})\s+(\S+)$') { $expected[$Matches[2]] = $Matches[1] }
    }
    if ($expected[$zipName] -ne (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()) {
        throw "Checksum mismatch for $zipName; nothing was installed."
    }
    $unpacked = Join-Path $work "unpacked"
    Expand-Archive -Path $zip -DestinationPath $unpacked
    $from = (Get-ChildItem $unpacked -Directory | Select-Object -First 1).FullName
    foreach ($file in Get-ChildItem $from -File | Where-Object { $_.Name -ne "SHA256SUMS" }) {
        if ($expected[$file.Name] -ne (Get-FileHash $file.FullName -Algorithm SHA256).Hash.ToLower()) {
            throw "Checksum mismatch for $($file.Name); nothing was installed."
        }
    }

    # 3. The install layout, with the automatic copies.
    New-Item -ItemType Directory -Force $bin, $libexec | Out-Null
    $full = Join-Path $from "process-tree-guard.exe"
    Copy-Item $full (Join-Path $bin "process-tree-guard.exe") -Force
    Copy-Item $full (Join-Path $bin "process-tree-guard-auto.exe") -Force
    $short = Join-Path $from "ptg.exe"
    if (Test-Path $short) {
        Copy-Item $short (Join-Path $bin "ptg.exe") -Force
        Copy-Item $short (Join-Path $bin "ptg-auto.exe") -Force
    }
    $deadline = Join-Path $from "process-tree-guard-deadline.exe"
    Copy-Item $deadline (Join-Path $libexec "process-tree-guard-deadline.exe") -Force
    Copy-Item $deadline (Join-Path $libexec "process-tree-guard-deadline-auto.exe") -Force
    foreach ($role in @("process-tree-guard-gate.exe", "process-tree-guard-contain.exe")) {
        Copy-Item (Join-Path $from $role) (Join-Path $libexec $role) -Force
    }
    foreach ($doc in @("POLICY.md", "LICENSE", "NOTICE", "README.md")) {
        Copy-Item (Join-Path $from $doc) (Join-Path $root $doc) -Force
    }
    Write-Host "Installed process-tree-guard $version to $root"

    # 4. PATH.
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $parts = @($userPath -split ";" | Where-Object { $_ })
    if (-not $env:PTG_NO_PATH -and $parts -notcontains $bin) {
        [Environment]::SetEnvironmentVariable("Path", (($parts + $bin) -join ";"), "User")
        Write-Host "Added $bin to your PATH. Open a new terminal to use it."
    }

    # 5. Hook configuration for the coding agents found on this machine.
    $exe = (Join-Path $bin "process-tree-guard.exe") -replace '\\', '/'
    $hooks = Join-Path $root "hooks"
    New-Item -ItemType Directory -Force $hooks | Out-Null
    $found = @()
    $home_ = $env:USERPROFILE
    if (Test-Path (Join-Path $home_ ".claude")) {
        $claude = @{ hooks = @{ PreToolUse = @(
            @{ matcher = "Bash"; hooks = @(@{ type = "command"; command = "$exe hook pretooluse"; timeout = 10 }) },
            @{ matcher = "PowerShell"; hooks = @(@{ type = "command"; command = "$exe hook powershell-auto"; timeout = 10 }) }
        ) } }
        Write-Json ($claude) (Join-Path $hooks "claude-code.json")
        $found += "Claude Code: merge hooks\claude-code.json into $home_\.claude\settings.json"
    }
    if (Test-Path (Join-Path $home_ ".codex")) {
        $codex = @{ hooks = @{ PreToolUse = @(
            @{ matcher = "Bash"; hooks = @(@{ type = "command"; command = "$exe hook pretooluse"; timeout = 10 }) }
        ) } }
        Write-Json ($codex) (Join-Path $hooks "codex.json")
        $found += "Codex: merge hooks\codex.json into your Codex hook settings"
    }
    if (Test-Path (Join-Path $home_ ".cursor")) {
        $cursor = @{ version = 1; hooks = @{
            beforeShellExecution = @(@{ command = "$exe hook cursor"; failClosed = $true; timeout = 10 })
            sessionStart = @(@{ command = "$exe hook cursor-context" })
        } }
        Write-Json ($cursor) (Join-Path $hooks "cursor.json")
        $found += "Cursor: merge hooks\cursor.json into $home_\.cursor\hooks.json"
    }
    if ((Test-Path (Join-Path $home_ ".gemini\antigravity")) -or (Test-Path (Join-Path $home_ ".antigravity"))) {
        $antigravity = @{
            "process-tree-guard" = @{ PreToolUse = @(@{ matcher = "run_command"; hooks = @(@{ type = "command"; command = "$exe hook antigravity"; timeout = 10 }) }) }
            "process-tree-guard-policy" = @{ SessionStart = @(@{ type = "command"; command = "$exe hook antigravity-context" }) }
        }
        Write-Json ($antigravity) (Join-Path $hooks "antigravity.json")
        $found += "Antigravity: merge hooks\antigravity.json into your Antigravity hook settings"
    }
    if ($found.Count -gt 0) {
        Write-Host "Coding agents found. Hook files with this machine's paths are in $hooks :"
        $found | ForEach-Object { Write-Host "  $_" }
        Write-Host "The integrations folder of the repository explains each host."
    } else {
        Write-Host "No coding agent settings found; see the integrations folder of the repository to add hooks later."
    }
    Write-Host "Try: process-tree-guard --timeout 60 -- npm test"
} finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
