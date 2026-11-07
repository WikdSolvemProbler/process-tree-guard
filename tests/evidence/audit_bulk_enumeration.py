#!/usr/bin/env python3
"""Offline archive/raw consistency audit; never extracts or launches artifacts."""
import hashlib
import json
from pathlib import Path
import struct
import zipfile

ARCHIVE_SHA256 = 'c4dcc4578071029c9da39a02fae4091eb3929b5296e0a30a2aad96f99cf289b6'

def sha(raw):
    return hashlib.sha256(raw).hexdigest()

def audit(path):
    assert sha(path.read_bytes()) == ARCHIVE_SHA256, 'archive identity differs'
    with zipfile.ZipFile(path) as z:
        names = z.namelist()
        assert len(names) == len(set(names)) == 88 and z.testzip() is None
        index = json.loads(z.read('INDEX.json'))
        assert set(names) == set(index) | {'INDEX.json'}
        for name, pin in index.items():
            raw = z.read(name)
            assert len(raw) == pin['bytes'] and sha(raw) == pin['sha256'], name
        def record(name):
            return json.loads(z.read(name))
        def rows(name):
            return [json.loads(line) for line in z.read(name).splitlines()]
        for receipt_name, prefix in (('unit-receipt-001.json', 'bulk/'),
                                     ('receipt-001.json', 'saved-decoder/')):
            receipt = record(prefix + receipt_name)
            assert receipt['qualified'] is True and receipt['code'] == 0
            for name, pin in receipt['artifact_sha256'].items():
                assert sha(z.read(prefix + name)) == pin
        seal = record('bulk/native-seal.json')
        for name, pin in seal['source_sha256'].items():
            assert sha(z.read('bulk/' + name)) == pin, name
        proposal = record('saved-decoder/proposal.json')
        for name, pin in proposal['source_sha256'].items():
            assert sha(z.read('saved-decoder/' + name)) == pin
        for path_name, pin in proposal['reference_sha256'].items():
            assert sha(z.read('bulk/' + Path(path_name).name)) == pin
        native = rows('bulk/native-001.jsonl')
        observed = [r for r in native if r['kind'] == 'snapshot_observed']
        snapshots = [r for r in native if r['kind'] == 'snapshot']
        assert len(observed) == len(snapshots) == 1
        observation, snapshot, terminal = observed[0], snapshots[0], native[-1]
        assert terminal['kind'] == 'terminal' and terminal['qualified'] is True
        assert terminal['code'] == 0 and terminal['native_capture_attempts'] == 1
        assert terminal['native_observation_count'] == 1 and terminal['controller_identity_validated'] is True
        assert terminal['baseline_handles'] == terminal['final_handles'] == 165
        assert terminal['observed_ns'] < terminal['deadline_ns']
        assert not terminal['cleanup_errors'] and terminal['primary_failure'] is None
        assert terminal['module_cleanup_failure'] is None and not terminal['reporting_failures']
        for key in ('census_qualified', 'phase_qualified', 'campaign_qualified', 'benchmark_slots'):
            assert not terminal[key]
        evidence = observation['evidence']
        header = evidence['native']
        assert header['count'] == len(observation['rows']) == len(evidence['raw_utf16_rows']) == 524
        assert header['status'] == header['primary_error'] == header['close_error'] == header['clock_error'] == 0
        assert header['upper_ticks'] == observation['upper_ticks'] > 0
        keys = ('begin_qpc', 'snapshot_upper_begin_qpc', 'snapshot_upper_end_qpc',
                'enumerate_begin_qpc', 'enumerate_end_qpc', 'close_begin_qpc', 'close_end_qpc', 'end_qpc')
        chain = [header[k] for k in keys]
        assert min(chain) > 0 and chain == sorted(chain)
        assert evidence['begin_ns'] <= evidence['native_begin_ns'] <= evidence['native_end_ns'] <= evidence['end_ns']
        wire = bytearray()
        pids = set()
        for actual, raw in zip(observation['rows'], evidence['raw_utf16_rows']):
            assert actual['pid'] == raw['pid'] and actual['parent_pid'] == raw['parent_pid']
            assert actual['pid'] not in pids
            pids.add(actual['pid'])
            prefix = bytes.fromhex(raw['name_utf16le_hex'])
            assert 1 <= raw['name_units'] < 260 and len(prefix) == 2 * raw['name_units']
            name = prefix.decode('utf-16le', 'surrogatepass')
            assert '\0' not in name and name.casefold() == actual['name']
            wire.extend(struct.pack('<III520s', raw['pid'], raw['parent_pid'], raw['name_units'], prefix))
        assert snapshot['observer_pid'] in pids and snapshot['guard_pid'] in pids
        assert 0 < snapshot['observer_created_ticks'] <= observation['upper_ticks']
        assert 0 < snapshot['guard_created_ticks'] <= observation['upper_ticks']
        assert snapshot['held_observer_alive'] and snapshot['held_guard_alive']
        assert bytes(wire) == z.read('saved-decoder/saved-wire-001.bin')
        semantic_raw = z.read('saved-decoder/saved-semantic-output-001.json')
        semantic = json.loads(semantic_raw)
        assert semantic['ok'] is True
        semantic_rows, upper, semantic_evidence = semantic['result']
        assert semantic_rows == observation['rows'] and upper == observation['upper_ticks']
        assert semantic_evidence['native'] == header
        assert semantic_evidence['raw_utf16_rows'] == evidence['raw_utf16_rows']
        saved = rows('saved-decoder/comparison-001.jsonl')
        measured = [r for r in saved if r['kind'] == 'measured']
        assert [(r['pair'], r['variant']) for r in measured] == [
            (0, 'old'), (0, 'new'), (1, 'new'), (1, 'old'),
            (2, 'old'), (2, 'new'), (3, 'new'), (3, 'old')]
        full_pin = sha(semantic_raw.rstrip(b'\n'))
        for r in measured:
            assert r['matches_saved_facts'] and r['full_output_sha256'] == full_pin
            assert r['wall_ns'] == r['wall_end_ns'] - r['wall_begin_ns'] >= 0
            assert r['process_cpu_ns'] == r['cpu_end_ns'] - r['cpu_begin_ns'] >= 0
        cases = [r for r in saved if r['kind'] == 'adversarial']
        assert len(cases) == 16 and len({r['case'] for r in cases}) == 16
        assert sum(r['expected_accept'] for r in cases) == 2
        for r in cases:
            assert r['full_output_equal'] and r['old_output_sha256'] == r['new_output_sha256']
        receipt = record('saved-decoder/receipt-001.json')
        assert receipt['python_tests'] == {'tests': 11, 'failures': 0, 'errors': 0, 'skipped': 0}
        assert receipt['paired_calls'] == 8 and receipt['adversarial_cases'] == 16
        assert receipt['receipt_observed_ns'] < receipt['deadline_ns'] and not receipt['errors']
        for key in ('private_dll_loads', 'native_snapshot_calls', 'benchmark_slots',
                    'qualified_native_capture', 'qualified_census', 'qualified_phase'):
            assert not receipt[key]
    return {'archive_members': 88, 'native_rows': 524, 'saved_paired_calls': 8,
            'adversarial_cases': 16, 'python_tests': 11, 'benchmark_slots': 0,
            'scope': 'Offline byte/raw consistency only; per-call output equality rests on reviewed enforcement and logged digests. No native replay, full-census admission or guard comparison.'}

if __name__ == '__main__':
    print(json.dumps(audit(Path(__file__).resolve().parent / 'windows-native-bulk-enumeration-001.zip'), indent=2))
