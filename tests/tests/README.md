<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# tests

## Contents

- [tests](#tests)

This folder holds the test files of the suite. Each test file holds one family or one subject of the completeness accounting.

**Belongs here:** The test files of the suite and the two helper files that they read.

**Does not belong here:** Campaign or battery code. The test files read that code; they do not hold it.

`guard_driver.py` finds and runs the built guard programs for the guard families.

`scanner_reference.py` is the reference form of the streaming scanner. The scanner tests compare the guard with this form.

**Next:** The [suite manual](../README.md) states how to run the suite.

[↑ Back to Contents](#contents)
