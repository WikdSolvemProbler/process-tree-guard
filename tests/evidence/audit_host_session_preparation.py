#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Audit saved benchmark preparation, without executing its scripts or binaries."""
import hashlib,json,re,zipfile
from pathlib import Path
EXPECTED='72fcac301fd3ea3279bed07fd18d25d37d8e2750bf9702d5267a0a6be602cacb'
def sha(data):return hashlib.sha256(data).hexdigest()
def need(ok,message):
 if not ok:raise SystemExit(message)
def main():
 file=Path(__file__).with_name('windows-host-session-preparation-001.zip')
 need(sha(file.read_bytes())==EXPECTED,'archive changed')
 with zipfile.ZipFile(file)as archive:
  names=archive.namelist();need(len(names)==len(set(names))==151,'member coverage')
  need(archive.testzip()is None,'CRC mismatch')
  index=json.loads(archive.read('INDEX.json'))
  need(set(names)==set(index)|{'INDEX.json'},'index coverage')
  for name,pin in index.items():
   raw=archive.read(name);need(len(raw)==pin['bytes']and sha(raw)==pin['sha256'],name)
  for prefix,count,pin in [('host-source004-history',21,'44daaf54415c22c8ace6ff3460c4c270139b4c5c5c67e62895d3607d27a0add7'),('host-source005',24,'c8d9937418fc2d981373178aeb4efb3cf312d1eaaad4aa27d62481013fe21b8a')]:
   raw=archive.read(prefix+'/pure-receipt-001.json');need(sha(raw)==pin,'receipt changed')
   receipt=json.loads(raw);need(receipt['code']==0 and receipt['qualified']is True,'pure failure')
   need(receipt['facts']=={'tests':count,'failures':0,'errors':0,'skipped':0},'pure counts')
   need(receipt['observed_ns']<receipt['deadline_ns'],'pure deadline')
   log=archive.read(prefix+'/pure-001.log');need(sha(log)==receipt['log_sha256'],'log binding')
   names_ok=re.findall(rb'^test_[^\r\n]+\.\.\. ok\r?$',log,re.M)
   need(len(names_ok)==count,'actual pass entries')
   need(archive.read(prefix+'/unit-plan.json')==archive.read(prefix+'/unit-plan.review-candidate.json'),'consumed plan changed')
   plan=json.loads(archive.read(prefix+'/unit-plan.json'))
   for name,digest in plan['source_sha256'].items():need(sha(archive.read(prefix+'/'+name))==digest,'source binding '+name)
   for flag in ['whole_arm_custody_qualified','retention_native_qualified','session_approved']:need(receipt[flag]is False,'scope expanded')
  result=json.loads(archive.read('RESULTS.json'))
  for flag in ['performance_qualified','session_approved','whole_arm_custody_qualified','retention_native_qualified','dispatcher_native_qualified','primary_comprehensive_campaign_complete']:need(result[flag]is False,'false qualification')
  need(result['comparative_arms_attempted']==result['native_controls_run']==0,'unexpected comparative/native results')
  need(sha(archive.read('current-guard/process-tree-guard.exe'))==result['current_guard_sha256'],'current guard differs')
 print(json.dumps({'members':151,'pure_phases':[21,24],'native_or_comparative_qualified':False,'benchmark_slots':0,'scope':'Offline byte/result audit; phases are successive sources, not45 independent cases'}))
if __name__=='__main__':main()
