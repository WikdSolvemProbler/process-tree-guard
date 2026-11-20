# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
# Dot-source before launching unattended Windows test/build children. The mode
# is process-local and inherited during child loading; it changes no registry
# setting, exit status, captured stream, deadline or production executable.
if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {
    if (-not ('ProcessTreeGuard.Tests.ErrorMode' -as [type])) {
        Add-Type -TypeDefinition @'
using System.Runtime.InteropServices;
namespace ProcessTreeGuard.Tests {
    public static class ErrorMode {
        [DllImport("kernel32.dll")] public static extern uint GetErrorMode();
        [DllImport("kernel32.dll")] public static extern uint SetErrorMode(uint mode);
    }
}
'@
    }
    $testErrorMode = [ProcessTreeGuard.Tests.ErrorMode]::GetErrorMode() -bor 0x8003
    [void][ProcessTreeGuard.Tests.ErrorMode]::SetErrorMode($testErrorMode)
    if (([ProcessTreeGuard.Tests.ErrorMode]::GetErrorMode() -band $testErrorMode) -ne $testErrorMode) {
        throw 'Could not suppress unattended test error dialogs; no tests may launch.'
    }
}
