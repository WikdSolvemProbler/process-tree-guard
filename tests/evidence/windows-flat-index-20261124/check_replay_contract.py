"""Synthetic verifier contract checks, not benchmark measurements."""
import hashlib
import json
from pathlib import Path
import runpy
import sys

cases = [
("hit-refund-1",8192),("hit-refund-32",8192),("hit-refund-1024",8192),
("fill-ascending",32768),("fill-descending",32768),
("first-full-miss",32),("backed-off-miss",8192),("one-dead-churn",128),
("mostly-retaining-expiry",32),("alternating-retain",32),
("full-expiry",32),("shutdown",32)]
orders = [(0,1,1,0),(1,0,0,1),(1,0,0,1),(0,1,1,0)]
parser = Path(sys.argv[1])
root = Path(sys.argv[2])
if root.exists():
    raise ValueError("Synthetic fixture namespace already exists")
root.mkdir()
lines = ["ROW,block,visit,arm,case,ns,operations"]
for block in range(17):
    order = (0,1) if block == 0 else orders[(block-1)%4]
    for visit,arm in enumerate(order):
        for name,ops in cases:
            lines.append(f"ROW,{block},{visit},{arm},{name},{(100-arm)*ops},{ops}")
reports = [f"CASE,{name},median_num={-4*ops},median_den={4*ops},p90_num={-2*ops},p90_den={2*ops},pass=true" for name,ops in cases]
terminal = "COST_SCREEN,pass=true,blocks=16,cases=12"
streams = {
"known-minus-one":lines+reports+[terminal],
"terminal-before-complete":lines+reports[:1]+[terminal]+reports[1:],
"missing-terminal":lines+reports,
}
verify = runpy.run_path(str(parser))["verify"]
results = []
for name,values in streams.items():
    source=root/(name+".csv")
    source.write_text("\n".join(values)+"\n",encoding="ascii")
    output=root/(name+".json")
    if name=="known-minus-one":
        result=verify(source,output)
        assert result["cost_screen_pass"] is True and result["rows"]==792
        assert all(row["median_ns_per_operation"]=="-1" and row["p90_ns_per_operation"]=="-1" for row in result["cases"])
        results.append({"fixture":name,"result":"accepted; all12 known analytical deltas=-1ns/operation"})
    else:
        try:
            verify(source,output)
        except ValueError as error:
            assert not output.exists()
            results.append({"fixture":name,"result":"refused","reason":str(error)})
        else:
            raise AssertionError("Invalid transcript accepted: "+name)
receipt={"scope":"Synthetic verifier fixtures only, no empirical measurement or guard test",
         "parser_sha256":hashlib.sha256(parser.read_bytes()).hexdigest(),"results":results}
(root/"RESULT.json").write_text(json.dumps(receipt,indent=2)+"\n",encoding="ascii")
print(json.dumps(receipt))

