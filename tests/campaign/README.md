<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# campaign

## Contents

- [campaign](#campaign)

This folder holds the campaign code of the suite. The code builds the slots of a campaign, runs them, and writes the report in the terms of the record.

**Belongs here:** The harness, the runner, the report, and the pricing code of the campaign. The counted-ops census data also lives here.

**Does not belong here:** Test files. Test files live in `tests/`.

`census.json` is the vendored counted-ops census. The census tests hold it to the tree's measurement record.

The empty folders in this folder are fixtures. An empty folder is itself the test condition. They carry no README, by design.

**Next:** The [suite manual](../README.md) states the campaign as a whole.

[↑ Back to Contents](#contents)
