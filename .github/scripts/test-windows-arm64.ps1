# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
if ($env:RUNNER_OS -ne 'Windows' -or $env:RUNNER_ARCH -ne 'ARM64' -or !$env:RUNNER_TEMP) {
    throw 'Run this test on a native Windows ARM64 Actions runner.'
}

function Assert-Arm64Pe($Path) {
    $reader = [IO.BinaryReader]::new([IO.File]::OpenRead($Path))
    try {
        if ($reader.ReadUInt16() -ne 0x5A4D) { throw "Invalid DOS signature in $Path" }
        $reader.BaseStream.Position = 0x3C
        $reader.BaseStream.Position = $reader.ReadUInt32()
        if ($reader.ReadUInt32() -ne 0x4550 -or $reader.ReadUInt16() -ne 0xAA64) {
            throw "Expected an ARM64 PE executable at $Path"
        }
    } finally {
        $reader.Dispose()
    }
}

# The released guard owns the Job deadline while we test the current build.
$asset = 'process-tree-guard-0.5.0-aarch64-pc-windows-msvc.zip'
# The immutable asset API avoids the release-download gateway; the digest below still pins its bytes.
$url = 'https://api.github.com/repos/WikdSolvemProbler/process-tree-guard/releases/assets/711818509'
$sha256 = 'eca8fb7ad0abb2d9c72b99446a12011b6e7130288d1612da55a868694edc098a'
$scratch = Join-Path $env:RUNNER_TEMP ("ptg-arm64-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $scratch | Out-Null
$zip = Join-Path $scratch $asset
$http = [Net.Http.HttpClient]::new()
try {
    $http.Timeout = [TimeSpan]::FromSeconds(45)
    $http.MaxResponseContentBufferSize = 8MB
    $http.DefaultRequestHeaders.UserAgent.ParseAdd('process-tree-guard-ci')
    $http.DefaultRequestHeaders.Accept.ParseAdd('application/octet-stream')
    $response = $http.GetAsync($url).GetAwaiter().GetResult()
    try {
        $null = $response.EnsureSuccessStatusCode()
        [IO.File]::WriteAllBytes($zip, $response.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult())
    } finally {
        $response.Dispose()
    }
} finally {
    $http.Dispose()
}
if ((Get-FileHash -Algorithm SHA256 -LiteralPath $zip).Hash -ne $sha256) {
    throw 'Unexpected released ARM64 guard archive digest.'
}
$bootstrap = Join-Path $scratch 'process-tree-guard.exe'
$entryPath = [IO.Path]::GetFileNameWithoutExtension($asset) + '/process-tree-guard.exe'
$archive = [IO.Compression.ZipFile]::OpenRead($zip)
try {
    $entries = @($archive.Entries | Where-Object { $_.FullName.Replace('\', '/') -ceq $entryPath })
    if ($entries.Count -ne 1) { throw "Expected one $entryPath in the release archive." }
    [IO.Compression.ZipFileExtensions]::ExtractToFile($entries[0], $bootstrap)
} finally {
    $archive.Dispose()
}
Assert-Arm64Pe $bootstrap

# Cargo identifies the test executable for this checkout and target.
Remove-Item Env:PROCESS_TREE_GUARD_EXE, Env:PROCESS_GUARD_EXE -ErrorAction SilentlyContinue
$cargo = (Get-Command cargo.exe -CommandType Application).Source
$buildArgs = @('test', '--release', '--locked', '--target', 'aarch64-pc-windows-msvc',
    '--test', 'guard', '--no-run', '--message-format', 'json')
$messages = & $bootstrap --timeout 240 -- $cargo @buildArgs
if ($LASTEXITCODE -ne 0) { throw "Bounded ARM64 test build exited $LASTEXITCODE" }
$executables = @(
    $messages | ConvertFrom-Json | Where-Object {
        $_.reason -eq 'compiler-artifact' -and $_.target.name -eq 'guard' -and
        $_.target.kind -contains 'test' -and $_.profile.test -and $_.executable
    } | ForEach-Object executable
)
if ($executables.Count -ne 1) { throw 'Expected one ARM64 guard integration test artifact.' }
Assert-Arm64Pe $executables[0]

$tests = @(
    'g01_missing_timeout_never_runs'
    'g11_to_g15_invalid_timeout'
    'g16_streams_exit_code_and_stdin'
    'g17_arguments_survive_quoting'
    'g18_timeout'
    'g19_g20_descendant_killed_on_timeout_and_normal_exit'
)
$output = & $bootstrap --timeout 90 -- $executables[0] --exact --color never --test-threads 1 @tests
$result = $LASTEXITCODE
$output | Write-Output
if ($result -ne 0) { throw "Bounded ARM64 integration tests exited $result" }
foreach ($test in $tests) {
    if ($output -cnotcontains "test $test ... ok") { throw "Missing successful result for $test" }
}
if (-not ($output -match '^test result: ok\. 6 passed; 0 failed; 0 ignored;')) {
    throw 'Unexpected native integration test totals.'
}
