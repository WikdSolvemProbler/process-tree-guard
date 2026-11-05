<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# certificates

## Contents

- [certificates](#certificates)

This folder holds the certificate record of each test in this package. A full run of the suite writes one certificate for each test in the run.

**Belongs here:** One certificate file for each test, and the index file of the run.

**Does not belong here:** Test code. A certificate is a record of a run. It is not a test.

`index.json` states the run and the tree SHA. It also states the count of certificates.

A certificate states the name of the test and the outcome of the test. It also states the SHA of the tree of the run. A certificate for a benchmark test also states the competitor programs of the test.

A test is complete only when a certificate at the current tree SHA states a pass for the test. A certificate at an old SHA states no pass for the current tree.

**Next:** The [manual](../README.md) states how to run the suite.

[↑ Back to Contents](#contents)
