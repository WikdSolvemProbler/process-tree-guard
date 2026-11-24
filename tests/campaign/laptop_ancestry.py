# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Exact instance allowance for the two mandatory enclosing guard roles."""

from pathlib import Path


ROLES = ("primary_guard", "tool_guard")
OUTSIDE_VETO = "pinned_workload_outside_job"
PRIMARY_STATE = "pinned_enclosing_guard"


def require(ok, message):
    if not ok:
        raise ValueError(message)


def role_instances(proof):
    return [
        {key: entry[key] for key in ("role", "pid", "path", "creation_filetime")}
        for entry in proof["roles"]
    ]


def validate_ancestry(proof, image_policy, observer_pid):
    """Reconstruct the reported parent chain and compare both roles to sealed pins."""
    require(type(proof) is dict and set(proof) == {"chain", "roles", "snapshot_upper_ticks"},
            "missing exact enclosing process ancestry")
    pins = image_policy["enclosing_guards"]
    chain, roles, upper = proof["chain"], proof["roles"], proof["snapshot_upper_ticks"]
    require(type(chain) is list and len(chain) >= 3 and type(roles) is list
            and len(roles) == len(ROLES) and type(upper) is int and upper > 0,
            "missing exact enclosing process ancestry")
    node_keys = {"pid", "parent_pid", "name", "path", "creation_filetime"}
    require(all(type(node) is dict and set(node) == node_keys for node in chain)
            and all(type(item) is dict and set(item) == node_keys | {"role", "image_sha256"}
                    for item in roles), "invalid enclosing ancestry receipt shape")
    require(type(observer_pid) is int and observer_pid > 0 and chain[0]["pid"] == observer_pid,
            "enclosing ancestry starts at a different observer")
    require(all(type(node["pid"]) is int and node["pid"] > 0
                and type(node["parent_pid"]) is int and node["parent_pid"] >= 0
                and type(node["creation_filetime"]) is int
                and 0 < node["creation_filetime"] <= upper
                and Path(node["path"]).name.casefold() == node["name"]
                for node in chain), "invalid process ancestry node")
    require(len({node["pid"] for node in chain}) == len(chain)
            and all(left["parent_pid"] == right["pid"]
                    and right["creation_filetime"] <= left["creation_filetime"]
                    for left, right in zip(chain, chain[1:])),
            "broken native parent or creation chain")
    require(tuple(item["role"] for item in roles) == ROLES and set(pins) == set(ROLES),
            "required enclosing guard roles differ")
    by_pid = {node["pid"]: (index, node) for index, node in enumerate(chain)}
    positions = []
    for item in roles:
        role, pid = item["role"], item["pid"]
        pin = pins[role]
        require(type(pid) is int and pid in by_pid
                and Path(item["path"]) == Path(pin["path"])
                and item["image_sha256"] == pin["sha256"]
                and pin["sha256"] in image_policy["veto_sha256"]
                and Path(pin["path"]).name.casefold() in image_policy["pinned_basenames"],
                "mandatory guard differs from its sealed image and veto")
        index, node = by_pid[pid]
        require(item == {**node, "role": role, "image_sha256": pin["sha256"]},
                "guard role differs from its exact ancestor node")
        positions.append(index)
    require(0 < positions[0] < positions[1] == len(chain) - 1,
            "mandatory guard instances are outside the ordered parent chain")


def census_admissions(rows, proof, before, after):
    """Derive the single exact-role allowance without changing raw classifications."""
    expected = role_instances(proof)
    require(before == expected == after, "held guards changed around the census")
    require(len({row["pid"] for row in rows}) == len(rows), "duplicate census process identity")
    primary_pid = proof["roles"][0]["pid"]
    require([row["pid"] for row in rows if row["state"] == PRIMARY_STATE] == [primary_pid],
            "census has an absent or extra primary guard identity")
    by_pid = {row["pid"]: row for row in rows}
    admissions = []
    for role in proof["roles"]:
        pid = role["pid"]
        require(pid in by_pid, "required held guard is absent from the census")
        row = by_pid[pid]
        require(type(row["pid"]) is int and type(row["parent_pid"]) is int
                and row["parent_pid"] == role["parent_pid"] and row["name"] == role["name"]
                and type(row["created_ticks"]) is int
                and row["created_ticks"] == role["creation_filetime"]
                and type(row["snapshot_upper_ticks"]) is int
                and row["created_ticks"] <= row["snapshot_upper_ticks"],
                "census row is not the exact held guard")
        if role["role"] == "primary_guard":
            require(row["state"] == PRIMARY_STATE, "primary guard lost its raw identity")
        else:
            require(row["state"] == OUTSIDE_VETO and row.get("alive_after") is True
                    and Path(row.get("image_path", "")) == Path(role["path"])
                    and row.get("image_sha256") == role["image_sha256"],
                    "raw tool-guard observation does not prove the sole ancestry admission")
            admissions.append({
                "role": role["role"], "pid": pid, "creation_filetime": role["creation_filetime"],
                "raw_state": OUTSIDE_VETO, "image_sha256": role["image_sha256"],
            })
    return admissions


def census_allowed(rows, admissions, allowed_states):
    """Leave every original non-ancestor veto in force."""
    return all(
        row["state"] in allowed_states or any(
            admission["pid"] == row["pid"] and admission["raw_state"] == row["state"]
            for admission in admissions
        )
        for row in rows
    )


class HeldGuardAncestry:
    """Use the original handles retained by the enclosing launch ExitStack."""

    def __init__(self, proof, handles, image_policy, observer_pid):
        validate_ancestry(proof, image_policy, observer_pid)
        self.proof = proof
        self.handles = handles
        self.pins = image_policy["enclosing_guards"]

    def primary_guard(self):
        role = self.proof["roles"][0]
        return role["pid"], self.handles["primary_guard"], role["creation_filetime"]

    def verify(self):
        from inventory_gate import identity, is_alive
        from laptop_barrier import native

        _, kernel, _ = native()
        records = []
        for role in self.proof["roles"]:
            name = role["role"]
            handle = self.handles[name]
            require(is_alive(handle), "a held enclosing guard is no longer live")
            image, created = identity(handle)
            pid = int(kernel.GetProcessId(handle))
            require(pid == role["pid"] and created == role["creation_filetime"]
                    and image == Path(role["path"]) and image.samefile(self.pins[name]["path"])
                    and is_alive(handle), "held enclosing guard image or identity changed")
            records.append({"role": name, "pid": pid, "path": str(image), "creation_filetime": created})
        return records

