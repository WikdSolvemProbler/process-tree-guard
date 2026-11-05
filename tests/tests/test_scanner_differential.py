# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""The streaming-scanner differential (FAILURE-TREE section 3;
MEASUREMENT section 3 assigns this suite the durable form).

The guard's hook surfaces decide verdicts with a streaming scanner
over a 16 KiB window; the tree's reference form parses the whole
payload and checks the parsed value. `scanner_reference.py` is an
independent implementation of the parsed form, written from the
tree's documented envelope semantics. This family:

- pins the reference to the documented verdicts on directed cases
  (anywhere tier);
- generates the seeded corpus and reports its size as the family's
  corpus size — each generated case is a comparison inside the
  differential tests below, never a test of its own (anywhere tier
  for generation and the reference side);
- runs the negative controls: a deliberately misaligned reference
  and a stub guard side that always allows must both be caught by
  the comparison (anywhere tier);
- drives the built guard binaries (full and gate roles, the
  pretooluse and cursor vendors) over every corpus case generated
  against the staged install layout and compares each verdict
  against the reference (Windows tier, skipped where the binaries
  are absent, like the other guard families).

The corpus covers the boundary shapes MEASUREMENT section 3
names: segment walks (the envelope paths, including the gate's
folder rules), prefix forms (the `& ` prefix and the exact
whitespace set at both edges), wrapper shapes (the automatic
wrapper's path is not an envelope), window edges (multibyte
scalars and escapes straddling the 16 KiB fills, long plain runs,
a watched string after a 100,000-byte unwatched string), timeout
tokens at both rounding bounds, and generated mutations and fuzz.
Payload-cap shapes are excluded here by construction (every case
is below the cap); the cap is the payload-cap family's subject.
"""

import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import guard_driver as drv  # noqa: E402
import scanner_reference as ref  # noqa: E402

PRESENT = drv.guard_present()
needs_guard = unittest.skipUnless(
    PRESENT, "guard binaries not built here (Windows package); see README tiers")

LAYOUT = ref.canonical_layout()
PATHS = ref.layout_paths(LAYOUT)
CORPUS = ref.generate_corpus(LAYOUT)


def staged_layout():
    """The corpus layout for the staged package: both roles share
    the build folder, so the gate's folder is the full binary's."""
    full = drv.locate("full").replace("\\", "/")
    gate = drv.locate("gate").replace("\\", "/")
    folder = full.rsplit("/", 1)[0]
    return ref.Layout(folder, gate), full, gate


def compare(cases, guard_side, reference_side):
    """Run both sides over `cases` (a list of (payload, cursor,
    exe) triples) and return the list of divergences."""
    divergences = []
    for index, (payload, cursor, exe) in enumerate(cases):
        expected = reference_side(payload, cursor, exe)
        got = guard_side(payload, cursor, exe)
        if got != expected:
            divergences.append((index, cursor, exe, expected, got, payload[:120]))
    return divergences


class ReferenceDirectedTests(unittest.TestCase):
    """The reference itself, pinned to the documented verdicts."""

    def verdict(self, command, exe=None, cursor=False):
        payload = ref._cur(command) if cursor else ref._pre(command)
        return ref.parsed_verdict(payload, cursor, exe or PATHS["full"])

    def test_documented_good_envelopes_are_allowed(self):
        full = PATHS["full"]
        for command in (
            ref._envelope(full),
            "& " + ref._envelope(full),
            "  " + ref._envelope(full) + " \n\t",
            ref._envelope(full, shell="bash"),
            ref._envelope(full, shell="cmd"),
            ref._envelope(full, timeout="0.5"),
            ref._envelope(full, timeout="86400"),
            ref._envelope(PATHS["alias"]),
        ):
            self.assertTrue(self.verdict(command), command)

    def test_documented_bad_envelopes_are_denied(self):
        full = PATHS["full"]
        for command in (
            "echo hi",
            ref._envelope(full, shell="zsh"),
            ref._envelope(full, timeout="86401"),
            ref._envelope(full, timeout="0"),
            ref._envelope(full, payload=""),
            ref._envelope(full, payload="IA=="),   # whitespace only
            ref._envelope(full, payload="AA=="),   # NUL
            ref._envelope(PATHS["gate"]),          # full never names the gate
            ref._envelope(full.upper().replace("C:", "c:")),
            ref._envelope(full.replace("/", "\\")),
            full + " --timeout  60 --shell powershell --command-b64 " + ref.B64_OK,
        ):
            self.assertFalse(self.verdict(command), command)

    def test_role_dependent_paths(self):
        gate = PATHS["gate"]
        # The deadline name is an envelope path for the gate role
        # (including in the bin folder beside a libexec gate) and
        # never for the full role.
        for path in (PATHS["deadline"], PATHS["gate_deadline"], PATHS["gate_alias"]):
            command = ref._envelope(path)
            self.assertFalse(self.verdict(command), path)
            self.assertTrue(self.verdict(command, exe=gate), path)
        # The gate never accepts its own path; the full binary's
        # path is the gate's bin sibling and is accepted.
        self.assertFalse(self.verdict(ref._envelope(gate), exe=gate))
        self.assertTrue(self.verdict(ref._envelope(PATHS["full"]), exe=gate))

    def test_timeout_token_boundaries(self):
        cases = {
            "60": True, "0.5": True, "86400": True, "86401": False,
            "0": False, ".5": False, "5.": False, "1e3": False,
            "86400." + ref.TIMEOUT_MID: True,
            "86400." + ref.TIMEOUT_MID[:-1] + "6": False,
            "0" * 100 + "60": True,
            "6" + "0" * 120: False,
            "0." + "0" * 323 + "3": True,
            "0." + "0" * 330 + "5": False,
        }
        for token, expected in cases.items():
            self.assertEqual(ref.timeout_token_ok(token), expected, token)

    def test_last_key_wins_and_cmd_is_only_a_fallback(self):
        import json as _json
        good = ref._envelope(PATHS["full"])
        quoted = _json.dumps(good)
        # command present (even non-string) shadows cmd entirely.
        payload = ('{"tool_input":{"command":7,"cmd":' + quoted + "}}").encode()
        self.assertFalse(ref.parsed_verdict(payload, False, PATHS["full"]))
        payload = ('{"tool_input":{"cmd":' + quoted + "}}").encode()
        self.assertTrue(ref.parsed_verdict(payload, False, PATHS["full"]))
        # A repeated tool_input replaces the earlier one whole.
        payload = ('{"tool_input":{"cmd":' + quoted +
                   '},"tool_input":{"command":"echo no"}}').encode()
        self.assertFalse(ref.parsed_verdict(payload, False, PATHS["full"]))
        # A multibyte folder spelling is matched character for character.
        exe = "C:/T\u00f6ols/pg/bin/process-guard.exe"
        payload = ref._pre(ref._envelope(exe, shell="bash"))
        self.assertTrue(ref.parsed_verdict(payload, False, exe))
        self.assertFalse(ref.parsed_verdict(payload, False, PATHS["full"]))


class CorpusTests(unittest.TestCase):
    def test_corpus_is_deterministic_and_below_the_cap(self):
        again = ref.generate_corpus(LAYOUT)
        self.assertEqual(again, CORPUS)
        self.assertTrue(all(len(c.payload) < ref.MAX_INPUT for c in CORPUS))

    def test_corpus_payload_hash_matches_the_summary(self):
        import hashlib
        import json as _json
        with open(os.path.join(HERE, "..", "evidence",
                               "scanner-corpus-summary.json"),
                  encoding="utf-8") as handle:
            summary = _json.load(handle)
        self.assertIn("8-byte big-endian",
                      summary["payloads_sha256_serialization"])
        digest = hashlib.sha256()
        for case in ref.generate_corpus(ref.canonical_layout()):
            digest.update(len(case.payload).to_bytes(8, "big"))
            digest.update(case.payload)
        self.assertEqual(digest.hexdigest(), summary["payloads_sha256"])

    def test_corpus_summary_fields_match_the_regenerated_corpus(self):
        # Round-22 adjudication P5: the recorded summary was held
        # only at its payload hash — its corpus size, category
        # counts, maximum payload, verdict tallies and comparison
        # count could all contradict the corpus it describes with
        # the suite green. Every field is recomputed here from
        # the regenerated corpus and held to the record.
        # Round-23: the summary's seed joins the recomputed set
        # (held to scanner_reference.CORPUS_SEED below).
        import json as _json
        from collections import Counter
        with open(os.path.join(HERE, "..", "evidence",
                               "scanner-corpus-summary.json"),
                  encoding="utf-8") as handle:
            summary = _json.load(handle)
        corpus = ref.generate_corpus(ref.canonical_layout())
        # Round-23: the seed joins the recomputed set — the
        # summary's recorded seed must be the CORPUS_SEED the
        # generator actually uses, or the recorded provenance
        # can claim a seed that produces a different corpus.
        self.assertEqual(summary["seed"], ref.CORPUS_SEED)
        self.assertEqual(summary["corpus_cases"], len(corpus))
        self.assertEqual(summary["categories"],
                         dict(Counter(c.category for c in corpus)))
        self.assertEqual(summary["max_payload_bytes"],
                         max(len(c.payload) for c in corpus))
        paths = ref.layout_paths(ref.canonical_layout())
        tallies = {}
        for role in ("full", "gate"):
            for vendor, cursor in (("pretooluse", False),
                                   ("cursor", True)):
                verdicts = [ref.parsed_verdict(
                    c.payload, cursor, paths[role]) for c in corpus]
                tallies[f"{role}/{vendor}"] = {
                    "allow": sum(verdicts),
                    "deny": len(verdicts) - sum(verdicts)}
        self.assertEqual(
            summary["reference_verdicts_canonical_layout"], tallies)
        window = [c for c in corpus
                  if c.category.startswith("window-")]
        self.assertEqual(
            summary["guard_side_comparisons_per_windows_run"],
            len(corpus) * 4 + len(window))

    def test_corpus_covers_the_named_categories(self):
        categories = {c.category for c in CORPUS}
        for named in ("directed", "directed-cursor", "timeout-token",
                      "structure", "mutation", "fuzz"):
            self.assertIn(named, categories)
        window = [c for c in CORPUS if c.category.startswith("window-")]
        self.assertGreaterEqual(len(window), 20)

    def test_corpus_verdicts_are_not_vacuous(self):
        # The reference side runs anywhere: over the canonical
        # layout, every role/vendor combination must produce both
        # verdicts, or the differential could pass by agreeing on
        # a constant.
        for exe in (PATHS["full"], PATHS["gate"]):
            for cursor in (False, True):
                verdicts = [ref.parsed_verdict(c.payload, cursor, exe)
                            for c in CORPUS]
                self.assertGreater(sum(verdicts), 0, (exe, cursor))
                self.assertLess(sum(verdicts), len(verdicts), (exe, cursor))


class NegativeControlTests(unittest.TestCase):
    """The comparison machinery must be able to fail (MEASUREMENT
    section 3 records a negative control for each executed
    differential it lists)."""

    def _triples(self):
        return [(c.payload, False, PATHS["full"]) for c in CORPUS]

    def test_misaligned_reference_is_caught(self):
        # A reference that matches envelope paths without regard to
        # letter case — the misalignment the section's controls use
        # in kind — must diverge from the aligned reference somewhere
        # in the corpus (the corpus holds an upper-cased path case).
        divergences = compare(
            self._triples(),
            lambda payload, cursor, exe: ref.parsed_verdict(payload, cursor, exe),
            lambda payload, cursor, exe: ref.parsed_verdict(
                payload, cursor, exe, path_case_insensitive=True))
        self.assertTrue(divergences)

    def test_always_allow_guard_side_is_caught(self):
        divergences = compare(
            self._triples(),
            lambda payload, cursor, exe: True,
            lambda payload, cursor, exe: ref.parsed_verdict(payload, cursor, exe))
        self.assertTrue(divergences)


@needs_guard
class ScannerDifferentialTests(unittest.TestCase):
    """Windows tier: the built binaries against the reference."""

    def _guard_side(self, role):
        def side(payload, cursor, exe):
            vendor = "cursor" if cursor else "pretooluse"
            if role == "gate":
                code, out, _, _ = drv.role("gate", [vendor], stdin=payload)
            else:
                code, out, _, _ = drv.guard(["hook", vendor], stdin=payload)
            return ref.guard_verdict(code, out, cursor)
        return side

    def test_scanner_matches_the_parsed_verdict_over_the_corpus(self):
        layout, full, gate = staged_layout()
        cases = ref.generate_corpus(layout)
        triples = []
        for case in cases:
            for exe in (full, gate):
                for cursor in (False, True):
                    triples.append((case.payload, cursor, exe))
        divergences = []
        sides = {"full": self._guard_side("full"), "gate": self._guard_side("gate")}
        for index, (payload, cursor, exe) in enumerate(triples):
            role = "gate" if exe == gate else "full"
            expected = ref.parsed_verdict(payload, cursor, exe)
            got = sides[role](payload, cursor, exe)
            if got != expected:
                divergences.append(
                    (index, role, cursor, expected, got, payload[:120]))
        self.assertEqual(
            divergences, [],
            f"{len(divergences)} divergences over {len(triples)} comparisons; "
            f"first: {divergences[:3]}")

    def test_scanner_matches_on_window_edge_cases(self):
        layout, full, _gate = staged_layout()
        cases = [c for c in ref.generate_corpus(layout)
                 if c.category.startswith("window-")]
        self.assertTrue(cases)
        divergences = compare(
            [(c.payload, False, full) for c in cases],
            self._guard_side("full"),
            lambda payload, cursor, exe: ref.parsed_verdict(payload, cursor, exe))
        self.assertEqual(divergences, [])


if __name__ == "__main__":
    unittest.main()
