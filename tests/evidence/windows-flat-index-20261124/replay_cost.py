"""Independent replay of the fixed flat-index internal cost screen."""
import csv
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import sys

CASES = {
    "hit-refund-1":8192, "hit-refund-32":8192, "hit-refund-1024":8192,
    "fill-ascending":32768, "fill-descending":32768,
    "first-full-miss":32, "backed-off-miss":8192, "one-dead-churn":128,
    "mostly-retaining-expiry":32, "alternating-retain":32,
    "full-expiry":32, "shutdown":32,
}
ORDERS = ((0,1,1,0),(1,0,0,1),(1,0,0,1),(0,1,1,0))
def require(value, message):
    if not value:
        raise ValueError(message)
def verify(source, destination):
    require(not destination.exists(), "replay output already exists")
    require(source.is_file() and source.stat().st_size <= 262144, "bounded raw output required")
    data = source.read_bytes()
    lines = data.decode("ascii", errors="strict").splitlines()
    require(lines[0] == "ROW,block,visit,arm,case,ns,operations", "header differs")
    rows = {}
    reports = {}
    reported_screen = None
    sequence = []
    section = "rows"
    for fields in csv.reader(lines[1:]):
        require(fields, "empty line")
        if fields[0] == "ROW":
            require(section == "rows" and len(fields) == 7, "row shape/order differs")
            block, visit, arm = map(int, fields[1:4])
            case = fields[4]
            ns, ops = map(int, fields[5:7])
            require(case in CASES and ops == CASES[case] and ns > 0, "invalid measurement")
            require(0 <= block <= 16, "block outside fixed plan")
            order = (0,1) if block == 0 else ORDERS[(block-1)%4]
            require(0 <= visit < len(order) and arm == order[visit], "visit/arm order differs")
            key = (block, visit, case)
            require(key not in rows, "duplicate row")
            rows[key] = ns
            sequence.append(key)
        elif fields[0] == "CASE":
            require(section in {"rows", "cases"}, "case after terminal report")
            section = "cases"
            require(len(fields) == 7 and fields[1] in CASES, "case report shape differs")
            case = fields[1]
            require(case not in reports, "duplicate case report")
            reports[case] = dict(item.split("=",1) for item in fields[2:])
        elif fields[0] == "COST_SCREEN":
            require(section == "cases" and len(fields) == 4 and reported_screen is None,
                    "screen report shape differs")
            require(list(reports) == list(CASES), "terminal report before all cases")
            reported_screen = dict(item.split("=",1) for item in fields[1:])
            section = "done"
        else:
            raise ValueError("unexpected raw line")
    require(section == "done", "terminal report missing")
    expected = []
    for block in range(17):
        visits = 2 if block == 0 else 4
        for visit in range(visits):
            expected.extend((block,visit,case) for case in CASES)
    require(sequence == expected and len(rows) == 792, "incomplete or reordered fixed plan")
    require(list(reports) == list(CASES), "missing/reordered case reports")
    results = []
    for case, ops in CASES.items():
        deltas = []
        for block in range(1,17):
            order = ORDERS[(block-1)%4]
            a = sum(rows[block,v,case] for v in range(4) if order[v] == 0)
            b = sum(rows[block,v,case] for v in range(4) if order[v] == 1)
            deltas.append(b-a)
        ordered = sorted(deltas)
        median = Fraction(ordered[7]+ordered[8],4*ops)
        p90 = Fraction(ordered[14],2*ops)
        passed = median <= 0 and p90 <= 0
        actual = reports[case]
        require(set(actual) == {"median_num","median_den","p90_num","p90_den","pass"},
                "unexpected case fields")
        require(int(actual["median_num"]) == ordered[7]+ordered[8]
                and int(actual["median_den"]) == 4*ops
                and int(actual["p90_num"]) == ordered[14]
                and int(actual["p90_den"]) == 2*ops
                and actual["pass"] == str(passed).lower(), "driver/replay disagreement")
        results.append({"case":case,"operations":ops,"paired_delta_sums_ns":deltas,
                        "median_ns_per_operation":str(median),
                        "p90_ns_per_operation":str(p90),"screen_pass":passed})
    passed = all(row["screen_pass"] for row in results)
    require(reported_screen == {"pass":str(passed).lower(),"blocks":"16","cases":"12"},
            "driver/replay screen disagreement")
    result = {"schema":"ptg-flat-index-cost-replay-v1","raw_sha256":hashlib.sha256(data).hexdigest(),
              "rows":792,"starter_rows":24,"measured_rows":768,"blocks":16,
              "behavior_oracles":"Reached complete raw plan under the frozen driver; no fake native liveness",
              "cost_screen_pass":passed,"cases":results,
              "scope":"Internal limiter cost only; no guard startup, native protection or equivalence claim"}
    destination.write_text(json.dumps(result,indent=2)+"\n",encoding="ascii")
    return result
if __name__ == "__main__":
    result = verify(Path(sys.argv[1]),Path(sys.argv[2]))
    print(json.dumps({"rows":result["rows"],"cost_screen_pass":result["cost_screen_pass"],
                      "failed_cases":[r["case"] for r in result["cases"] if not r["screen_pass"]]}))

