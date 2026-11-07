#!/usr/bin/env python3
"""Audit archived source/object provenance without extraction or execution."""
import base64
import hashlib
import io
import json
from pathlib import Path
import struct
import zipfile

ARCHIVE_SHA256 = '0848ccef7232bb14df277ab4e0ad4a65a637f7b020af3eb1ff1ad3da64b7c4eb'

def sha(raw):
    return hashlib.sha256(raw).hexdigest()

def audit(path):
    assert sha(path.read_bytes()) == ARCHIVE_SHA256
    with zipfile.ZipFile(path) as z:
        names = z.namelist()
        assert len(names) == len(set(names)) == 133 and z.testzip() is None
        index = json.loads(z.read('INDEX.json'))
        assert set(names) == set(index) | {'INDEX.json'}
        for name, pin in index.items():
            raw = z.read(name)
            assert len(raw) == pin['bytes'] and sha(raw) == pin['sha256'], name
        result = json.loads(z.read('RESULTS.json'))
        prefix = 'ordinary-compile-002/'
        assert result['attempt_exit'] == 0
        assert not result['production_backend_enabled'] and not result['performance_claim']
        assert not any(result['runtime_counts'].values())
        assert sha(z.read('source-candidate.json')) == result['source_candidate_sha256']
        assert sha(z.read(prefix + 'plan.json')) == result['plan_sha256']
        stdout = z.read(prefix + 'linux.stdout.log')
        assert sha(stdout) == result['raw_stdout_sha256']
        assert z.read(prefix + 'linux.stderr.log') == b''
        assert z.read(prefix + 'linux.ssh-exit.txt') == b'0'
        def records(label):
            return [json.loads(line.split('=', 1)[1]) for line in stdout.decode().splitlines()
                    if line.startswith(label + '=')]
        argv = records('NATIVE_ARGV')
        exits = records('NATIVE_EXIT')
        assert len(argv) == len(exits) == 6 and exits == result['native_steps']
        assert {r['tag'] for r in argv} == {'compiler-version', 'c-compiler-version',
            'terminal-bridge-dependencies', 'terminal-bridge-compile',
            'compile-main-object', 'compile-test-object'}
        for r in argv:
            args = r['argv']
            if r['tag'].startswith('compile-'):
                assert '--emit=obj' in args and '-D' in args and 'warnings' in args
            elif r['tag'] == 'terminal-bridge-compile':
                assert '-c' in args and '-Werror' in args and '-fstack-protector-strong' in args
        for r in exits:
            assert r['exit'] == 0
            assert z.read(prefix + 'download/' + r['tag'] + '.stderr.log') == b''
        header_raw = z.read(prefix + 'download/header-provenance.json')
        assert sha(header_raw) == result['c_header_provenance_sha256']
        header = json.loads(header_raw)['terminal-bridge']
        assert header['match'] and header['before'] == header['after']
        assert len(header['before']) == result['c_header_dependencies'] == 125
        for name, pin in result['objects'].items():
            raw = z.read(prefix + 'download/' + name)
            assert len(raw) == pin['bytes'] and sha(raw) == pin['sha256']
            assert raw[:6] == b'\x7fELF\x02\x01'
            assert struct.unpack_from('<HH', raw, 16) == (1, 183), 'ARM64 relocatable object required'
        preserve = json.loads(z.read(prefix + 'preserve.json'))
        remove = json.loads(z.read(prefix + 'remove.json'))
        assert preserve['mode'] == 'preserve' and not preserve['scratch_removed']
        assert remove['mode'] == 'remove' and remove['scratch_removed']
        assert preserve['original_scope_gone'] and remove['original_scope_gone']
        assert preserve['identity'] == remove['identity'] == [64769, 1111923]
        assert set(preserve['files']) == set(remove['files']) and len(remove['files']) == 49
        for name, pin in preserve['files'].items():
            assert {k: v for k, v in pin.items() if k != 'data_b64'} == remove['files'][name]
            assert base64.b64decode(pin['data_b64'], validate=True) == z.read(prefix + 'download/' + name)
        for pin in (preserve, remove):
            assert len(pin['recorded_workers_absent']) == 7
            assert all(not r['proc_entry_present'] for r in pin['recorded_workers_absent'])
        for name, pin in remove['files'].items():
            raw = z.read(prefix + 'download/' + name)
            assert len(raw) == pin['bytes'] and sha(raw) == pin['sha256']
        before = z.read(prefix + 'pre-removal.zip')
        assert sha(before) == result['preservation']['pre_removal_zip_sha256']
        with zipfile.ZipFile(io.BytesIO(before)) as pre:
            assert pre.testzip() is None and len(pre.namelist()) == 122
        complete = records('ORDINARY_COMPONENT_COMPLETE')
        assert len(complete) == 1 and complete[0]['prototype_executions'] == 0
    return {'members': 133, 'objects': 3, 'header_dependencies': 125,
            'preserved_artifacts': 49, 'recorded_pids_absent': 7,
            'scope': 'Object/source/provenance and archived cleanup consistency only; no runtime or guard performance qualification.'}

if __name__ == '__main__':
    print(json.dumps(audit(Path(__file__).resolve().parent / 'linux-terminal-controller-objects.zip'), indent=2))
