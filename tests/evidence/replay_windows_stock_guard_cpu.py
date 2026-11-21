"""Verify the sanitized stock Windows guard CPU diagnostic with exact arithmetic.

From the repository root, using Python 3.10+ and the configured bounded launcher:
    process-tree-guard --timeout 30 -- python tests/evidence/replay_windows_stock_guard_cpu.py

This script reads the adjacent JSON. It launches no processes and checks no
private receipts, build identities, or runtime oracles.
"""

import json
from collections import Counter
from fractions import Fraction
from pathlib import Path
from statistics import median


METRICS = (
    "elapsed_ns",
    "guard_kernel_100ns",
    "guard_user_100ns",
    "guard_total_100ns",
    "guard_cycles",
)
ROW_FIELDS = ("disposition", "block", "position", "cell", "image", "site", *METRICS)
SUMMARY_FIELDS = ("n", "median", "mean", "positive", "negative", "zero")
CONTRAST_FIELDS = ("balanced", "site0", "site1")
CELLS = {"A": ["A", 0], "B": ["B", 0], "C": ["A", 1], "D": ["B", 1]}
SEQUENCES = ("ABDC", "BCAD", "CDBA", "DACB")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def observations(report):
    layout = report["numerical_format"]
    require(layout["row_fields"] == list(ROW_FIELDS), "Unexpected row fields")
    require(layout["metric_order"] == list(METRICS), "Unexpected metric order")
    require(layout["summary_fields"] == list(SUMMARY_FIELDS), "Unexpected summary fields")
    require(layout["contrast_fields"] == list(CONTRAST_FIELDS), "Unexpected contrast fields")

    protocol = report["protocol"]
    require(protocol["cells"] == CELLS, "Unexpected cells")
    require(protocol["qualification_order"] == "ABDC", "Unexpected qualification order")
    require(protocol["measured_sequences"] == list(SEQUENCES), "Unexpected measured sequences")
    require(protocol["repeats"] == 2, "Unexpected sequence repeats")

    expected_slots = [("qualification_excluded", None, "ABDC")]
    expected_slots.extend(
        ("measured", block, order) for block, order in enumerate(SEQUENCES * 2)
    )
    raw_rows = report["observations"]
    require(len(raw_rows) == 36, "Expected 4 qualification rows and 32 measured rows")
    measured = []
    for index, values in enumerate(raw_rows):
        row = dict(zip(ROW_FIELDS, values, strict=True))
        disposition, block, order = expected_slots[index // 4]
        position = index % 4
        cell = order[position]
        require(
            [row["disposition"], row["block"], row["position"], row["cell"]]
            == [disposition, block, position, cell],
            f"Row {index}: unexpected slot or disposition",
        )
        require(
            [row["image"], row["site"]] == CELLS[cell],
            f"Row {index}: incorrect image or site",
        )
        for metric in METRICS:
            raw = row[metric]
            require(
                isinstance(raw, str) and raw.isascii() and raw.isdecimal(),
                f"Row {index}: invalid unsigned {metric}",
            )
            value = int(raw)
            require(
                raw == str(value) and (metric == "guard_total_100ns" or value < 2**64),
                f"Row {index}: invalid unsigned {metric}",
            )
            row[metric] = value
        require(
            row["guard_total_100ns"] == row["guard_kernel_100ns"] + row["guard_user_100ns"],
            f"Row {index}: CPU total does not equal kernel plus user",
        )
        if disposition == "measured":
            measured.append(row)
    for position in range(4):
        require(
            Counter(row["cell"] for row in measured if row["position"] == position)
            == Counter({"A": 2, "B": 2, "C": 2, "D": 2}),
            f"Unbalanced cells at position {position}",
        )
    return measured


def summarize(values):
    return [
        len(values),
        str(median(values)),
        str(sum(values) / len(values)),
        sum(value > 0 for value in values),
        sum(value < 0 for value in values),
        sum(value == 0 for value in values),
    ]


def recompute_blocks(measured):
    blocks = []
    for index in range(8):
        members = measured[index * 4 : (index + 1) * 4]
        order = "".join(row["cell"] for row in members)
        by_cell = {row["cell"]: row for row in members}
        contrasts = {}
        for metric in METRICS:
            site0 = by_cell["B"][metric] - by_cell["A"][metric]
            site1 = by_cell["D"][metric] - by_cell["C"][metric]
            contrasts[metric] = [str(Fraction(site0 + site1, 2)), str(site0), str(site1)]
        blocks.append(
            {
                "index": index,
                "order": order,
                "first_image": members[0]["image"],
                "site0_first_image": "A" if order.index("A") < order.index("B") else "B",
                "site1_first_image": "A" if order.index("C") < order.index("D") else "B",
                "contrasts": contrasts,
            }
        )
    return blocks


def recompute_summary(metric, measured, blocks):
    def contrast_stats(field, subset):
        position = CONTRAST_FIELDS.index(field)
        return summarize([Fraction(block["contrasts"][metric][position]) for block in subset])

    by_position = {}
    for position in range(4):
        groups = {
            image: [
                Fraction(row[metric])
                for row in measured
                if row["position"] == position and row["image"] == image
            ]
            for image in ("A", "B")
        }
        by_position[str(position)] = {
            image: summarize(values) for image, values in groups.items()
        }
        by_position[str(position)]["B_minus_A_marginal_mean"] = str(
            sum(groups["B"]) / len(groups["B"]) - sum(groups["A"]) / len(groups["A"])
        )

    return {
        **{field: contrast_stats(field, blocks) for field in CONTRAST_FIELDS},
        "sequence": {
            order: contrast_stats("balanced", [block for block in blocks if block["order"] == order])
            for order in SEQUENCES
        },
        "first_image": {
            image: contrast_stats("balanced", [block for block in blocks if block["first_image"] == image])
            for image in ("A", "B")
        },
        "within_site_order": {
            site: {
                image: contrast_stats(
                    site, [block for block in blocks if block[site + "_first_image"] == image]
                )
                for image in ("A", "B")
            }
            for site in ("site0", "site1")
        },
        "position": by_position,
        "measured_zero_count": sum(row[metric] == 0 for row in measured),
    }


def main():
    path = Path(__file__).with_name("windows-stock-guard-cpu-20261120.json")
    report = json.loads(path.read_text(encoding="utf-8"))
    require(report["schema"] == "windows-stock-guard-cpu-diagnostic-v1", "Unexpected schema")
    measured = observations(report)
    blocks = recompute_blocks(measured)
    require(blocks == report["blocks"], "Saved and recomputed block contrasts differ")
    require(set(report["summary"]) == set(METRICS), "Unexpected summary metric set")
    for metric in METRICS:
        require(
            recompute_summary(metric, measured, blocks) == report["summary"][metric],
            f"Saved and recomputed {metric} summaries differ",
        )
    print("Verified 36 observations, 4 excluded qualifications, and 8 blocks across 5 metrics.")


if __name__ == "__main__":
    main()
