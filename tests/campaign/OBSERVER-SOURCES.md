# Frozen Windows observer sources

The four files in `observer-laptop-182-001/` are byte-identical to the source set used by the sealed Stage011 study. Keep that directory limited to those four files and use isolated Python with bytecode writing disabled. The manifest binds their exact SHA-256 values.

These are benchmark observers. They do not change or install the Rust guard, start a background service, or run automatically. Every invocation needs its own finite process-tree deadline.

A011 recorded 36 controls and 148 warmups, then refused an unverified Python process during its first inspection. Its saved error does not identify the failed API or establish that the process exited. The observer remains strict; these published bytes contain no error-87 exception. The attempt remains incomplete with no measured trial or ranking. See [the failure record](../evidence/windows-laptop-attempt-011-failure-20261124-summary.json).
