#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Audit saved repair evidence; execute no archived program or native test."""
import hashlib
import json
from pathlib import Path
import re
import zipfile

EXPECTED_ARCHIVE = 'cc62ab5842f2139fd057af42f5f72a0c38bd2aa419f0287eaf9d23d2bc0af3c8'
SOURCE = '5eb80d5a01dbbe2720c54fb370058c382c28eb7f00958ea8cbc356d7d9d8f318'

def require(ok, text):
    if not ok:
        raise SystemExit(text)

def sha(data):
    return hashlib.sha256(data).hexdigest()

def main():
    file = Path(__file__).with_name('windows-cleanup-verdict-repair.zip')
    require(sha(file.read_bytes()) == EXPECTED_ARCHIVE, 'archive hash differs')
    with zipfile.ZipFile(file) as archive:
        names = archive.namelist()
        require(len(names) == len(set(names)), 'duplicate member')
        require(archive.testzip() is None, 'CRC differs')
        index = json.loads(archive.read('INDEX.json'))
        require(set(names) == set(index) | {'INDEX.json'}, 'index coverage differs')
        for name, pin in index.items():
            data = archive.read(name)
            require(len(data) == pin['bytes'] and sha(data) == pin['sha256'], name)
        source_bytes = archive.read('repair-evidence/source-candidate.json')
        require(sha(source_bytes) == SOURCE, 'source manifest differs')
        for name, pin in json.loads(source_bytes)['files'].items():
            require(sha(archive.read('current-repository/' + name)) == pin, 'changed source: ' + name)
        package = json.loads(archive.read('publication/PACKAGE.json'))
        require(package['required_validation_passed'] is True, 'required validation did not pass')
        require(package['extra_strict_clippy_passed'] is False and package['all_recorded_attempts_passed'] is False, 'strict failure hidden')
        require(package['performance_qualified'] is False and package['live_survivor_reproduced'] is False and package['benchmark_slots'] == 0, 'scope expanded')
        attempts = package['terminal_attempts']
        for name in ['release-lib-003', 'release-full-001', 'release-build-clippy-002']:
            require(attempts[name]['passed'] is True, 'required attempt failed: ' + name)
        require(attempts['release-build-clippy-001']['passed'] is False, 'strict failure rescored')
        full = archive.read('repair-evidence/release-full-001/stdout.log').decode('utf-8')
        summaries = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', full)
        require(sum(int(row[0]) for row in summaries) == 472, 'full test count differs')
        require(all(row[1:] == ('0', '0') for row in summaries), 'functional failure/skip')
        require(len(package['release_binaries']) == 6, 'binary role count')
        for role, pin in package['release_binaries'].items():
            data = archive.read(pin['member'])
            require(sha(data) == pin['sha256'] and len(data) == pin['bytes'], 'binary differs: ' + role)
        results = json.loads(archive.read('publication/RESULTS.json'))
        require(results['repair_commit'] == 'cf6045b04586914b41d8d28a88c13262b173a9c5', 'repair provenance differs')
        require(len(results['limitations']) >= 7 and results['lint']['zero_advisory_claim'] is False, 'limits lost')
    print(json.dumps({'members': len(names), 'functional_tests_passed': 472, 'required_local_checks_passed': True, 'optional_strict_clippy_passed': False, 'performance_qualified': False, 'benchmark_slots': 0, 'scope': 'offline archived-byte/result audit only; no native execution or immutable-source inference'}))

if __name__ == '__main__':
    main()
