#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Offline saved-artifact audit only; never executes archived code."""
import hashlib,json,struct,zipfile
from pathlib import Path
def sha(b):return hashlib.sha256(b).hexdigest()
def need(v,m):
 if not v:raise SystemExit(m)
def main():
 p=Path(__file__).with_name('linux-watchdog-arming-objects.zip')
 with zipfile.ZipFile(p) as z:
  names=z.namelist();need(len(names)==len(set(names)),'duplicate members');need(z.testzip() is None,'CRC')
  ix=json.loads(z.read('INDEX.json'));need(set(names)==set(ix)|{'INDEX.json'},'coverage')
  for n,v in ix.items():
   b=z.read(n);need(len(b)==v['bytes'] and sha(b)==v['sha256'],n)
  root='linux015/';r=json.loads(z.read('RESULTS.json'))
  need(sha(z.read(root+'source-candidate.json'))==r['source_candidate_sha256'],'source manifest')
  m=json.loads(z.read(root+'source-candidate.json'))
  for n,v in m['files'].items():need(sha(z.read(root+n))==v['sha256'],'source '+n)
  for n,v in m['external_dependencies'].items():need(sha(z.read('dependencies/'+n))==v['sha256'],'dependency '+n)
  d=root+'ordinary-compile-001/preservation-002/download/'
  for n in ['native-arming.o','kernel-signal-abi.o','activation-main.o','activation-tests.o']:
   b=z.read(d+n);need(b[:6]==b'\x7fELF\x02\x01' and struct.unpack_from('<HH',b,16)==(1,183),'AARCH64 relocatable '+n)
  need(len(r['steps'])==8 and all(x['code']=='0' for x in r['steps']),'steps')
  need(all(not x for x in r['qualification'].values()),'scope')
  before=json.loads(z.read(root+'ordinary-compile-001/preservation-002/preserve.json'))
  after=json.loads(z.read(root+'ordinary-compile-001/removal-003/remove.json'))
  need(len(before['files'])==len(after['files'])==191 and before['identity']==after['identity']==[64769,1111923],'preserved inventory')
  need(after['scratch_removed'] is True and after['original_scope_gone'] is True,'cleanup')
  need(len(after['recorded_workers_absent'])==9 and all(x['proc_entry_present'] is False for x in after['recorded_workers_absent']),'workers')
  for n,v in before['files'].items():need(sha(z.read(d+n))==v['sha256'],'download '+n)
  need(sha(z.read(root+'ordinary-compile-001/pre-removal.zip'))==r['preservation']['prearchive_sha256'],'prearchive')
 print(json.dumps({'members':len(names),'objects':4,'steps':8,'native_executions':0,'backend_qualified':False,'cleanup':'exact owned scratch removed'}))
if __name__=='__main__':main()
