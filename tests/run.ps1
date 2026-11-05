# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
# Run the full campaign, the battery, and the suite, writing
# results in the specification's terms.
#
#   .\run.ps1 [-Tree <path-to-guard-tree>]
#
# Preconditions: .\build.ps1 has staged build\; campaign.json in this
# folder names the staged guard (the anchor arm: the current gauntlet
# binary) and the competitor inputs as found. The run is ONE attempt:
# the reference plan as configured: 1,940 slots (36 control + 224
# warmup + 1,680 measured; 1,620 stands as the original shape's
# reference decomposition; the size is set by the measurement
# argument, not by quota), no retries, exclusions or replacements.
# Results land in results\ (observations.jsonl, run.json, report.txt), the
# battery's verdicts and per-case driver evidence records in
# results\battery\ (battery.json plus one battery-case-<id>.json per
# case) with the printed report in results\battery-report.json, the
# test families' summary in results\tests.txt, and every stage's
# exit code in results\stage-exit-codes.txt.
#
# Stage outcomes are not all the same kind of thing, and the script
# keeps them apart:
#
# - A campaign failure is an infrastructure abort: the measurement
#   did not happen, the script stops at once, and no results are
#   claimed complete.
# - A campaign INCOMPLETE outcome (including a freeze-gate FAIL)
#   is a failed stage, not an abort: the campaign measured, and its
#   own verdict is that it is not a result. The verdict is carried
#   by status, not by the harness exit code (the harness exits 0
#   for a completed execution whose outcome is incomplete): the
#   harness records campaign_status ("complete" / "incomplete"),
#   the standing-loss count and the freeze-gate FAIL count in the
#   run's run.json, and the same verdict in report.txt's STATUS
#   line. This script reads campaign_status from the newest
#   run.json under results\, records it in stage-exit-codes.txt as
#   "campaign status: ...", and names the campaign in the final
#   aggregate exactly as it names a battery FAIL decision, while
#   the battery and the test families still run. A campaign whose
#   status is missing or unrecorded is named too: this script
#   never exits 0 over a campaign whose outcome it cannot read.
# - A battery FAIL is a decision, not an abort. FAIL means a case
#   was refuted and its item is not adopted; that is the battery
#   doing its job. battery.py reports a FAIL decision with exit
#   code 3 (its exit-code contract), the verdicts stay recorded in
#   results\battery\, and the run CONTINUES to the test families.
#   The battery stage is named in the aggregate failure reported
#   after every stage has run.
# - A battery execution failure still aborts loudly, before the
#   test families: battery.py exit code 2 (a case BLOCKED-ERROR:
#   a driver missing, a driver error, evidence withheld), any
#   other unexpected battery exit code (for example a crash), or
#   no battery.json written into results\battery\ at all. The
#   battery did not produce decisions, so there is nothing to
#   continue on the strength of.
# - Test-family failures are recorded and named in the same
#   aggregate at the end; they never rewrite the earlier stages.
#
# One tree rule for build and run: -Tree defaults exactly as
# build.ps1's does (.. beside this script — the folder that
# holds this suite — when unnamed), and
# the battery audits the tree -Tree names — run.ps1 resolves it
# and passes it to battery.py as --tree, and every battery case
# record names the tree it audited. Before the campaign stage
# runs, run.ps1 compares the resolved tree with the staged build
# record build.ps1 wrote (build\build-record.json): on a mismatch
# it refuses, naming both trees, before any stage executes its
# payload, so the battery never silently audits a tree the build
# did not produce. When no build record exists, the run proceeds
# with -Tree governing the battery, and the stage record states
# the audited tree and that no build record was present to check
# against.
#
# Tier: Windows-theory from the Linux development machine (the same
# commands were exercised there in reduced form against stub arms).

param(
    [string]$Tree = (Join-Path $PSScriptRoot "..")
)
$ErrorActionPreference = "Stop"

# --- prerequisites -------------------------------------------------------
# Preflight, before stage 1: Python 3 on PATH is required, or the
# run dies later with a raw native error and no stage record.
# This failure names the missing prerequisite and its remedy.
if (-not (Get-Command python -ErrorAction SilentlyContinue)) { throw "missing prerequisite: Python 3 on PATH (python) not found; remedy: install Python 3 and ensure python is on PATH" }
& python --version | Out-Null
if ($LASTEXITCODE -ne 0) { throw "missing prerequisite: the python version check failed; remedy: install Python 3 and ensure python on PATH runs Python 3" }

$here = $PSScriptRoot
$results = Join-Path $here "results"
$stageRecord = Join-Path $results "stage-exit-codes.txt"

# The battery exit-code contract (battery\battery.py).
$BatteryExecutionExit = 2
$BatteryFailDecisionExit = 3

# The campaign outcome contract (campaign\harness.py): run.json's
# campaign_status field carries the completeness verdict.
$CampaignStatusComplete = "complete"
$CampaignStatusIncomplete = "incomplete"
$CampaignStatusKey = "campaign_status"

$failedStages = @()

# --- tree consistency -----------------------------------------------------
# The battery audits a tree's sources; the campaign measures the
# binaries build.ps1 staged. Those must be the same tree. Resolve
# the tree this run will audit (-Tree, defaulting exactly as
# build.ps1's), then, before any stage executes its payload,
# compare it with the staged build record (build\build-record.json).
# A mismatch refuses here, naming both trees. With no build record
# present there is nothing to check against: the run proceeds with
# -Tree governing the battery, and the stage record states the
# audited tree and that no build record was present to check
# against.
$treeAsGiven = $Tree
if (Test-Path -LiteralPath $Tree) {
    $treeResolved = (Resolve-Path -LiteralPath $Tree).Path
} elseif ([System.IO.Path]::IsPathRooted($Tree)) {
    $treeResolved = [System.IO.Path]::GetFullPath($Tree)
} else {
    $treeResolved = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot $Tree))
}
$buildRecordPath = Join-Path $here "build\build-record.json"
$buildRecordTree = $null
$buildRecordTreeResolved = $null
if (Test-Path $buildRecordPath) {
    $buildRecord = Get-Content $buildRecordPath -Raw | ConvertFrom-Json
    $buildRecordTree = $buildRecord.tree_as_given
    $buildRecordTreeResolved = $buildRecord.tree_resolved
    if ($buildRecordTreeResolved -ne $treeResolved) {
        throw "tree mismatch: the staged build record $buildRecordPath records a build of tree '$buildRecordTree' (resolved '$buildRecordTreeResolved'), but this run would audit tree '$treeAsGiven' (resolved '$treeResolved'); refusing before any stage runs - re-run with -Tree naming the built tree, or rebuild with build.ps1 -Tree naming this tree"
    }
}

# Stage 1: the campaign. A failure here aborts the whole run.
python (Join-Path $here "campaign\harness.py") run `
    --config (Join-Path $here "campaign.json") `
    --out $results
$campaignExit = $LASTEXITCODE
if ($campaignExit -ne 0) {
    # The stage record is written even when the harness failed
    # before creating the results folder — an admission refusal
    # writes no namespace — so the results folder is created
    # here first and the campaign exit code is always recorded.
    New-Item -ItemType Directory -Force $results | Out-Null
    Set-Content -Path $stageRecord -Value "campaign exit: $campaignExit"
    throw "campaign harness failed with $campaignExit; the run stops here and no results are complete"
}
Set-Content -Path $stageRecord -Value "campaign exit: $campaignExit"
Add-Content -Path $stageRecord -Value "audited tree: $treeResolved (as given: $treeAsGiven)"
if ($buildRecordTreeResolved) {
    Add-Content -Path $stageRecord -Value "build record tree check: matched the staged build record's tree '$buildRecordTreeResolved'"
} else {
    Add-Content -Path $stageRecord -Value "build record tree check: no build record present at $buildRecordPath; no build record was present to check against"
}

# The campaign's own outcome, read from the newest run namespace's
# run.json (campaign_status). An INCOMPLETE campaign — a freeze-gate
# FAIL included — is a failed stage named in the final aggregate,
# not an abort: the battery and the test families still run.
$campaignStatus = "unrecorded"
$campaignRunJson = Get-ChildItem -Path $results -Filter "run.json" -Recurse -File |
    Sort-Object LastWriteTime | Select-Object -Last 1
if ($campaignRunJson) {
    try {
        $campaignMeta = Get-Content $campaignRunJson.FullName -Raw | ConvertFrom-Json
        $recordedStatus = $campaignMeta.$CampaignStatusKey
        if ($recordedStatus) {
            $campaignStatus = $recordedStatus
        }
    } catch {
        $campaignStatus = "unrecorded"
    }
}
Add-Content -Path $stageRecord -Value "campaign status: $campaignStatus"
if ($campaignStatus -ne $CampaignStatusComplete) {
    $failedStages += "campaign (INCOMPLETE campaign: campaign_status '$campaignStatus' in run.json, standing losses in report.txt)"
    Write-Host "campaign outcome: INCOMPLETE (campaign_status '$campaignStatus'); recorded as a failed stage; continuing to the battery and the test families"
}

# Stage 2: the battery. A FAIL decision is recorded and the run
# continues; an execution failure aborts loudly, right here.
$batteryDir = Join-Path $results "battery"
python (Join-Path $here "battery\battery.py") --json `
    --tree $treeResolved `
    --results-dir $batteryDir |
    Tee-Object (Join-Path $results "battery-report.json")
$batteryExit = $LASTEXITCODE
Add-Content -Path $stageRecord -Value "battery exit: $batteryExit"
if (-not (Test-Path (Join-Path $batteryDir "battery.json"))) {
    throw "battery execution failed: no battery.json was written into results\battery; the battery did not run and the run stops here"
}
if ($batteryExit -eq $BatteryExecutionExit) {
    throw "battery execution failed with $batteryExit (BLOCKED-ERROR: a case could not be executed); the run stops here"
}
if ($batteryExit -ne 0 -and $batteryExit -ne $BatteryFailDecisionExit) {
    throw "battery execution failed with $batteryExit; the run stops here"
}
if ($batteryExit -eq $BatteryFailDecisionExit) {
    $failedStages += "battery (FAIL decision, recorded in results\battery)"
    Write-Host "battery decision: FAIL, recorded in results\battery; continuing to the test families"
}

# Stage 3: every test family. This stage runs whenever the campaign
# measured and the battery produced decisions, FAIL included.
# The test runner writes its progress to stderr by design, and
# this stage merges that stream into the recorded output. Under
# the Stop preference set at the top of this script, a native
# command whose stderr is redirected is surfaced as a
# terminating error in Windows PowerShell, which would abort
# the stage on the first progress line before its exit code
# could be read. The stage therefore runs under Continue and
# the script preference is restored immediately after; the
# exit code below is the stage's verdict, as before.
$ErrorActionPreference = "Continue"
python (Join-Path $here "run_tests.py") 2>&1 |
    Tee-Object (Join-Path $results "tests.txt")
$ErrorActionPreference = "Stop"
$testsExit = $LASTEXITCODE
Add-Content -Path $stageRecord -Value "test families exit: $testsExit"
if ($testsExit -ne 0) {
    $failedStages += "test families (exit $testsExit, recorded in results\tests.txt)"
}

if ($failedStages.Count -gt 0) {
    throw "run finished every stage; stages that did not pass: $($failedStages -join '; ')"
}

Write-Host "results in $results"
