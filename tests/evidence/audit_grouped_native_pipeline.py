#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Offline integrity and result audit of the preserved P2 diagnostic, not a rerun."""
import hashlib
import json
from pathlib import Path
import zipfile

EXPECTED_ZIP = 'be73309a694f63a408e63560c2c6c123fe22ee52b298cab30047b9799f4a46a3'
EXPECTED_RAW = '4092db50df3400e88e9c9010fcb15d9a8ad89c10d2cd758f4fa186774d0108f3'
PREFIX = 'inputs/issue7-grouped-image-pipeline-002/'

def require(condition, message):
    if not condition:
        raise SystemExit(message)

def sha(data):
    return hashlib.sha256(data).hexdigest()

def main():
    path = Path(__file__).with_name('windows-grouped-native-pipeline-002.zip')
    require(sha(path.read_bytes()) == EXPECTED_ZIP, 'archive hash changed')
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        require(len(names) == len(set(names)) == 153, 'member count or duplicate')
        require(archive.testzip() is None, 'CRC mismatch')
        index = json.loads(archive.read('INDEX.json'))
        require(set(names) == set(index) | {'INDEX.json'}, 'index coverage differs')
        for name, entry in index.items():
            require(not name.startswith('/') and '..' not in Path(name).parts, 'unsafe name')
            data = archive.read(name)
            require(len(data) == entry['bytes'] and sha(data) == entry['sha256'], name)
        for number in range(1, 5):
            require(any(name.startswith(PREFIX + f'pre-review-{number:03}/') for name in names), 'missing prior draft')
        raw = archive.read(PREFIX + 'diagnostic-001.jsonl')
        require(sha(raw) == EXPECTED_RAW, 'raw diagnostic changed')
        # Python integers retain original creation times and native identities.
        rows = [json.loads(line) for line in raw.splitlines()]
        samples = [row for row in rows if row['kind'] == 'sample']
        require([row['index'] for row in samples] == [0, 1, 2], 'snapshot order')
        expected = [40946100, 59638700, 48262100]
        require([row['strict_complete_ns'] for row in samples] == expected, 'strict timings')
        for row in samples:
            require(row['finished_ns'] - row['stamp_begin_ns'] == row['strict_complete_ns'], 'cutoff arithmetic')
            require(row['time_budget_pass'] == (row['strict_complete_ns'] <= 50000000), 'budget verdict')
            require(row['row_count'] == 26 and row['group_count'] == 4, 'census population')
        terminal = [row for row in rows if row['kind'] == 'terminal']
        require(len(terminal) == 1, 'terminal count')
        terminal = terminal[0]
        require(terminal['code'] == 0 and terminal['completed'] == 3, 'attempt did not finish')
        require(terminal['observed_ns'] < terminal['deadline_ns'], 'deadline exceeded')
        require(terminal['sources_unchanged'] is True, 'source changed')
        require(terminal['primary_failure'] is None and terminal['cleanup_failure'] is None, 'native failure')
        require(terminal['reporting_failures'] == [], 'reporting failure')
        for field in ['diagnostic_pipeline_qualified', 'phase_qualified', 'campaign_qualified']:
            require(terminal[field] is False, 'false qualification claim: ' + field)
        require(terminal['benchmark_slots'] == 0, 'unexpected benchmark slots')
        pure = json.loads(archive.read(PREFIX + 'unit-receipt-001.json'))
        require(pure['qualified'] is True and pure['code'] == 0 and pure['tests'] == 34, 'pure controls')
        require(pure['failures'] == pure['test_errors'] == pure['skipped'] == 0 and pure['errors'] == [], 'pure failures')
        require(pure['private_dll_loads'] == pure['native_snapshot_calls'] == pure['benchmark_slots'] == 0, 'pure scope')
        for member, expected_hash in pure['artifact_sha256'].items():
            require(sha(archive.read(PREFIX + member)) == expected_hash, 'pure log binding')
    print(json.dumps({'archive': path.name, 'verified_members': len(names), 'strict_ms': [n / 1e6 for n in expected], 'diagnostic_pipeline_qualified': False, 'campaign_qualified': False, 'benchmark_slots': 0, 'scope': 'offline preserved-byte integrity/result audit; not independent replay of every native predicate or new measurement'}))

if __name__ == '__main__':
    main()
