# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Independent parsed-form reference for the hook verdict, and the
seeded corpus the streaming-scanner differential runs over.

The reference is written from the tree's documented envelope
semantics — the module documentation and the whole-string forms in
`tree/src/hook.rs`, `tree/src/hook/envelope.rs` (the whole-string
`valid_envelope` / `envelope_for` / `envelope_remainder`),
`tree/src/hook/json.rs` (the parsed value tree), and
`tree/src/base64.rs` (`command_text_is_valid`) — and imports no
guard logic: the guard side of the differential is the built
binary, driven through its hook surfaces. The two sides therefore
cross independent implementations, as the tree's own differential
record intends (MEASUREMENT section 3: the streaming verdict
scanner against the parsed verdict, `parse_json` plus
`valid_envelope`).

Semantics implemented, in brief:

- The payload must be exactly one JSON value with nothing after it
  but the grammar's four whitespace bytes. For `pretooluse` the
  watched string is the last `tool_input` value's `command` (the
  `cmd` key only when no `command` key is present at all); for
  `cursor` it is the top-level `command`. A repeated key's last
  value wins, in keys as in values.
- The watched string must be exactly the guard envelope:
  optional leading `& `, one envelope path, ` --timeout `, a
  timeout token, ` --shell `, one of powershell / bash / cmd,
  ` --command-b64 `, and a payload that decodes under strict
  Base64 to nonempty UTF-8 with no NUL and at least one
  non-whitespace character. Leading and trailing Unicode
  whitespace (the White_Space property set) is trimmed first.
- Envelope paths: for the full role, the executable itself and the
  other of its two names in the same folder; for the gate role,
  the deadline-enforcing names in the gate's folder and, when that
  folder is named `libexec` in any letter case, in the `bin`
  folder beside it — never the gate's own path. Path matching is
  exact, character for character (letter case included).
- The timeout token: digits with at most one point, a nonempty
  whole part, a nonempty fraction when a point is present, and a
  value above zero and at most 86400. A token of at most 32 bytes
  is settled by parsing it; a longer token is settled by the
  counter rules (significant whole digits, and the fraction's
  comparison against the half-ULP bound above 86400 and the
  half-subnormal bound above zero).
- The parsed verdict knows no payload cap: it judges the whole
  string it is handed. The guard's streaming side caps its read
  at 16 MiB, so the differential corpus stays below the cap; the
  cap itself is the payload-cap family's subject.

Corpus hazards discipline: shapes where a whole-string parse and
a streaming walk are documented to be able to part company in
unwatched positions (malformed numbers, exotic escapes) are
placed, as in the tree's own payload list, in payloads whose
verdict is deny on either reading, so the comparison stays a
comparison of verdicts.
"""

import base64
import json
import random
from collections import namedtuple

MAX_INPUT = 16 * 1024 * 1024
WINDOW = 16384
MAX_SECONDS = 86400
TIMEOUT_MID = "0000000000072759576141834259033203125"
ZERO_MID = "24703282292062327"
SHELL_NAMES = ("powershell", "bash", "cmd")
NAMES = ("process-tree-guard", "process-guard")
DEADLINE = "process-tree-guard-deadline"
GATE_SUFFIX = "/process-tree-guard-gate.exe"

# The Unicode White_Space property set: the characters
# `char::is_whitespace` holds, and the set both trims use.
WHITESPACE = frozenset(
    [chr(c) for c in range(0x09, 0x0E)]
    + [" ", "\u0085", "\u00a0", "\u1680"]
    + [chr(c) for c in range(0x2000, 0x200B)]
    + ["\u2028", "\u2029", "\u202f", "\u205f", "\u3000"]
)
JSON_WS = " \t\n\r"
_B64_ALPHABET = frozenset(
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/")


def _ascii_lower(text):
    return "".join(
        chr(ord(c) + 32) if "A" <= c <= "Z" else c for c in text)


def _trim(text):
    start, end = 0, len(text)
    while start < end and text[start] in WHITESPACE:
        start += 1
    while end > start and text[end - 1] in WHITESPACE:
        end -= 1
    return text[start:end]


# ------------------------------------------------------------- JSON

class JObj:
    __slots__ = ("pairs",)

    def __init__(self, pairs):
        self.pairs = pairs


class JArr:
    __slots__ = ("items",)

    def __init__(self, items):
        self.items = items


_FAIL = object()


class _Parser:
    """The whole-string JSON parse, mirroring the tree's parser:
    depth capped at 256 values, the grammar's four whitespace
    bytes, strict strings (no raw control characters, surrogate
    pairs joined, a lone surrogate refused)."""

    def __init__(self, text):
        self.text = text
        self.pos = 0
        self.depth = 0

    def ws(self):
        text = self.text
        while self.pos < len(text) and text[self.pos] in JSON_WS:
            self.pos += 1

    def eat(self, ch):
        self.ws()
        if self.pos < len(self.text) and self.text[self.pos] == ch:
            self.pos += 1
            return True
        return False

    def value(self):
        self.ws()
        self.depth += 1
        if self.depth > 256:
            return _FAIL
        try:
            if self.pos >= len(self.text):
                return _FAIL
            ch = self.text[self.pos]
            if ch == "{":
                return self._object()
            if ch == "[":
                return self._array()
            if ch == '"':
                return self._string()
            if ch == "t":
                return self._literal("true", True)
            if ch == "f":
                return self._literal("false", False)
            if ch == "n":
                return self._literal("null", None)
            return self._number()
        finally:
            self.depth -= 1

    def _literal(self, word, value):
        if self.text.startswith(word, self.pos):
            self.pos += len(word)
            return value
        return _FAIL

    def _object(self):
        self.pos += 1
        pairs = []
        if not self.eat("}"):
            while True:
                self.ws()
                key = self._string()
                if key is _FAIL:
                    return _FAIL
                if not self.eat(":"):
                    return _FAIL
                val = self.value()
                if val is _FAIL:
                    return _FAIL
                pairs.append((key, val))
                if self.eat(","):
                    continue
                if not self.eat("}"):
                    return _FAIL
                break
        return JObj(pairs)

    def _array(self):
        self.pos += 1
        items = []
        if not self.eat("]"):
            while True:
                val = self.value()
                if val is _FAIL:
                    return _FAIL
                items.append(val)
                if self.eat(","):
                    continue
                if not self.eat("]"):
                    return _FAIL
                break
        return JArr(items)

    def _number(self):
        start = self.pos
        text = self.text
        while self.pos < len(text) and text[self.pos] in "-+.eE0123456789":
            self.pos += 1
        token = text[start:self.pos]
        if not token:
            return _FAIL
        try:
            return float(token)
        except ValueError:
            return _FAIL

    def _hex4(self):
        raw = self.text[self.pos:self.pos + 4]
        if len(raw) < 4:
            return None
        digits = raw[1:] if raw.startswith("+") else raw
        if not digits or any(c not in "0123456789abcdefABCDEF" for c in digits):
            return None
        self.pos += 4
        return int(digits, 16)

    def _string(self):
        if self.pos >= len(self.text) or self.text[self.pos] != '"':
            return _FAIL
        self.pos += 1
        out = []
        text = self.text
        while True:
            if self.pos >= len(text):
                return _FAIL
            ch = text[self.pos]
            self.pos += 1
            if ch == '"':
                return "".join(out)
            if ch == "\\":
                if self.pos >= len(text):
                    return _FAIL
                esc = text[self.pos]
                self.pos += 1
                if esc == '"':
                    out.append('"')
                elif esc == "\\":
                    out.append("\\")
                elif esc == "/":
                    out.append("/")
                elif esc == "b":
                    out.append("\b")
                elif esc == "f":
                    out.append("\f")
                elif esc == "n":
                    out.append("\n")
                elif esc == "r":
                    out.append("\r")
                elif esc == "t":
                    out.append("\t")
                elif esc == "u":
                    unit = self._hex4()
                    if unit is None:
                        return _FAIL
                    if 0xD800 <= unit < 0xDC00:
                        if text[self.pos:self.pos + 2] != "\\u":
                            return _FAIL
                        self.pos += 2
                        low = self._hex4()
                        if low is None or not (0xDC00 <= low < 0xE000):
                            return _FAIL
                        unit = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00)
                    elif 0xDC00 <= unit < 0xE000:
                        return _FAIL
                    out.append(chr(unit))
                else:
                    return _FAIL
            elif ord(ch) <= 0x1F:
                return _FAIL
            else:
                out.append(ch)


def parse_json(text):
    """One JSON value and nothing after it but whitespace, or None."""
    parser = _Parser(text)
    value = parser.value()
    if value is _FAIL:
        return None
    parser.ws()
    if parser.pos != len(parser.text):
        return None
    return value


def _get(value, key):
    if isinstance(value, JObj):
        for k, v in reversed(value.pairs):
            if k == key:
                return v
    return None


def _has(value, key):
    return isinstance(value, JObj) and any(k == key for k, _ in value.pairs)


# ------------------------------------------------- envelope checking

def timeout_token_ok(token):
    """The timeout token rules, counters included (see module doc)."""
    bad = False
    dot = False
    whole_digits = 0
    whole_value = 0
    whole_significant = 0
    whole_nonzero = False
    fraction_digits = 0
    fraction_mid = 0
    fraction_sub = 0
    for c in token:
        if c == "." and not dot:
            dot = True
        elif "0" <= c <= "9":
            digit = ord(c) - 48
            if dot:
                fraction_digits += 1
                place = fraction_digits
                if fraction_mid == 0:
                    bound = (ord(TIMEOUT_MID[place - 1]) - 48
                             if place <= len(TIMEOUT_MID) else 0)
                    if digit < bound:
                        fraction_mid = -1
                    elif digit > bound:
                        fraction_mid = 1
                if fraction_sub == 0:
                    if place < 324:
                        bound = 0
                    elif place - 324 < len(ZERO_MID):
                        bound = ord(ZERO_MID[place - 324]) - 48
                    else:
                        bound = 0
                    if digit < bound:
                        fraction_sub = -1
                    elif digit > bound:
                        fraction_sub = 1
            else:
                whole_digits += 1
                if digit != 0:
                    whole_nonzero = True
                if digit != 0 or whole_significant > 0:
                    if whole_significant < 5:
                        whole_value = whole_value * 10 + digit
                    whole_significant = min(whole_significant + 1, 6)
        else:
            bad = True
    if bad or whole_digits == 0 or (dot and fraction_digits == 0):
        return False
    if len(token.encode("utf-8")) <= 32:
        try:
            value = float(token)
        except ValueError:
            return False
        return 0.0 < value <= float(MAX_SECONDS)
    if whole_significant >= 6 or whole_value > MAX_SECONDS:
        return False
    if whole_significant == 5 and whole_value == MAX_SECONDS:
        return fraction_mid <= 0
    if whole_nonzero:
        return True
    return fraction_sub > 0


def command_text_is_valid(payload):
    """Strict Base64 whose decoded bytes are valid UTF-8 with no
    NUL and at least one non-whitespace character."""
    raw = payload.encode("utf-8")
    if not raw or len(raw) % 4 != 0:
        return False
    padding = 0
    for byte in reversed(raw):
        if byte == 0x3D:  # '='
            padding += 1
        else:
            break
    padding = min(padding, 3)
    if padding > 2:
        return False
    body = raw[:len(raw) - padding] if padding else raw
    if any(byte not in _B64_ALPHABET for byte in body):
        return False
    try:
        decoded = base64.b64decode(raw, validate=True)
    except ValueError:
        return False
    if b"\x00" in decoded:
        return False
    try:
        text = decoded.decode("utf-8")
    except UnicodeDecodeError:
        return False
    return any(c not in WHITESPACE for c in text)


def _family(exe):
    if "/" not in exe:
        return None
    folder, name = exe.rsplit("/", 1)
    if len(name) < 4 or _ascii_lower(name[-4:]) != ".exe":
        return None
    stem = name[:-4]
    for index, known in enumerate(NAMES):
        if _ascii_lower(stem) == known:
            return folder, index
    return None


def envelope_paths(exe, gate):
    """The paths an envelope may name for executable `exe`."""
    if gate:
        if "/" not in exe:
            return []
        folder = exe.rsplit("/", 1)[0]
        folders = [folder]
        if "/" in folder:
            root, last = folder.rsplit("/", 1)
            if _ascii_lower(last) == "libexec":
                folders.append(root + "/bin")
        paths = []
        for base in folders:
            for name in (NAMES[0], NAMES[1], DEADLINE):
                paths.append(f"{base}/{name}.exe")
        return [p for p in paths if _ascii_lower(p) != _ascii_lower(exe)]
    paths = [exe]
    found = _family(exe)
    if found is not None:
        folder, own = found
        paths.append(f"{folder}/{NAMES[1 - own]}.exe")
    return paths


def _envelope_remainder(rest):
    split = rest.find(" ")
    if split < 0:
        return False
    token, rest = rest[:split], rest[split + 1:]
    if not timeout_token_ok(token):
        return False
    if not rest.startswith("--shell "):
        return False
    rest = rest[len("--shell "):]
    payload = None
    for name in SHELL_NAMES:
        if rest.startswith(name):
            tail = rest[len(name):]
            if tail.startswith(" --command-b64 "):
                payload = tail[len(" --command-b64 "):]
                break
    if payload is None:
        return False
    padding = 0
    for c in reversed(payload):
        if c == "=":
            padding += 1
        else:
            break
    padding = min(padding, 3)
    if padding > 2 or len(payload.encode("utf-8")) == padding:
        return False
    return command_text_is_valid(payload)


def _envelope_for(command, path, case_insensitive=False):
    rest = _trim(command)
    if rest.startswith("& "):
        rest = rest[2:]
    if case_insensitive:
        if _ascii_lower(rest[:len(path)]) != _ascii_lower(path):
            return False
    elif not rest.startswith(path):
        return False
    rest = rest[len(path):]
    if not rest.startswith(" --timeout "):
        return False
    return _envelope_remainder(rest[len(" --timeout "):])


def valid_envelope(command, exe, gate, case_insensitive=False):
    return any(
        _envelope_for(command, path, case_insensitive)
        for path in envelope_paths(exe, gate))


def parsed_verdict(payload, cursor, exe, *, path_case_insensitive=False):
    """The parsed-form verdict on one hook payload (bytes).

    `exe` is the executable spelling the guard resolves for itself
    (forward slashes); the gate role follows its file name, as in
    the tree. An empty spelling denies, as the tree's reference
    holds. Invalid UTF-8 denies, as a failed whole-string read
    denies.
    """
    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError:
        return False
    value = parse_json(text)
    if value is None or not exe:
        return False
    if cursor:
        command = _get(value, "command")
    else:
        tool = _get(value, "tool_input")
        if _has(tool, "command"):
            command = _get(tool, "command")
        else:
            command = _get(tool, "cmd")
    if not isinstance(command, str):
        return False
    return valid_envelope(command, exe, exe.endswith(GATE_SUFFIX),
                          path_case_insensitive)


def guard_verdict(exit_code, stdout, cursor):
    """The guard binary's verdict from one hook invocation's result.
    Anything outside the documented allow/deny shapes is an error,
    never silently read as a verdict."""
    if cursor:
        if exit_code == 0 and json.loads(stdout) == {"permission": "allow"}:
            return True
        if exit_code == 2:
            answer = json.loads(stdout)
            if answer.get("permission") == "deny":
                return False
        raise AssertionError(
            f"cursor hook answered outside its shapes: exit {exit_code}, {stdout!r}")
    if exit_code == 0 and stdout.strip() == b"{}":
        return True
    if exit_code == 2:
        return False
    raise AssertionError(
        f"pretooluse hook answered outside its shapes: exit {exit_code}, {stdout!r}")


# ------------------------------------------------------------- corpus

#: An install layout the corpus is generated against: the folder
#: holding the full binary, and the gate binary's own path (in a
#: `libexec` folder in the canonical layout, so the gate's `bin`
#: sibling rule is exercised; in the staged layout, the same
#: folder as the full binary).
Layout = namedtuple("Layout", ["folder", "gate"])
Case = namedtuple("Case", ["category", "payload"])

CORPUS_SEED = 20261029
B64_OK = "ZWNobyBPSw=="  # "echo OK"


def canonical_layout():
    return Layout("C:/Tools/ptg/bin", "C:/Tools/ptg/libexec/process-tree-guard-gate.exe")


def layout_paths(layout):
    folder = layout.folder
    gate_folder = layout.gate.rsplit("/", 1)[0]
    return {
        "full": f"{folder}/process-tree-guard.exe",
        "alias": f"{folder}/process-guard.exe",
        "deadline": f"{folder}/process-tree-guard-deadline.exe",
        "gate": layout.gate,
        "gate_deadline": f"{gate_folder}/process-tree-guard-deadline.exe",
        "gate_alias": f"{gate_folder}/process-guard.exe",
        "auto": f"{folder}/process-tree-guard-auto.exe",
    }


def _envelope(path, timeout="60", shell="powershell", payload=B64_OK):
    return f"{path} --timeout {timeout} --shell {shell} --command-b64 {payload}"


def _pre(command):
    return json.dumps({"tool_input": {"command": command}}).encode("utf-8")


def _pre_cmd(command):
    return json.dumps({"tool_input": {"cmd": command}}).encode("utf-8")


def _cur(command):
    return json.dumps({"command": command}).encode("utf-8")


def _directed_commands(paths):
    full, alias = paths["full"], paths["alias"]
    good = _envelope(full)
    b64 = base64.b64encode
    return [
        good,
        "& " + good,
        "  " + good + " \n\t",
        "\n" + good,
        "\u00a0" + good,                       # NBSP is White_Space: trimmed
        good + "\u3000",                       # ideographic space: trimmed
        "\x0b" + good,                         # vertical tab is not: denied
        good + "\x0b",
        _envelope(full, shell="bash"),
        _envelope(full, shell="cmd"),
        _envelope(full, timeout="0.5"),
        _envelope(full, timeout="86400"),
        _envelope(full, timeout="00060"),
        _envelope(full, timeout="60.5"),
        _envelope(alias),
        _envelope(paths["deadline"]),          # gate-allowed, full-denied
        _envelope(paths["gate_deadline"]),     # gate-allowed, full-denied
        _envelope(paths["gate_alias"]),        # gate-allowed, full-denied
        _envelope(paths["gate"]),              # a gate never names itself
        _envelope(paths["auto"]),              # the wrapper is no envelope
        _envelope("C:/Other/bin/process-tree-guard.exe"),
        _envelope(full.upper().replace("C:", "c:")),  # case differs: denied
        _envelope(full.replace("/", "\\")),    # backslashes: denied
        _envelope(full[:-4]),                  # no .exe: denied
        _envelope(full + ".evil"),
        full + " --timeout  60 --shell powershell --command-b64 " + B64_OK,
        full + " --timeout 60  --shell powershell --command-b64 " + B64_OK,
        _envelope(full, shell="zsh"),
        _envelope(full, shell="POWERSHELL"),
        _envelope(full, shell="powershellx"),
        _envelope(full, payload=""),
        _envelope(full, payload="Zg=="),       # decodes to "f": allowed
        _envelope(full, payload="Zm8="),       # decodes to "fo": allowed
        _envelope(full, payload="Zm8"),        # not whole quartets
        _envelope(full, payload="IA=="),       # decodes to a space: denied
        _envelope(full, payload="AA=="),       # decodes to NUL: denied
        _envelope(full, payload="Zg==Zg=="),   # interior padding
        _envelope(full, payload=B64_OK + "!"),
        _envelope(full, timeout="60."),
        _envelope(full, timeout=".5"),
        _envelope(full, timeout="1e3"),
        _envelope(full, timeout="86401"),
        _envelope(full, timeout="0"),
        _envelope(full, timeout="0.0"),
        "x" + good,
        good + " extra",
        "&" + good,                            # & without its space
        "&  " + good,                          # & and two spaces
        "&& " + good,
        _envelope(full, payload=b64("echo \u00fc \U0001f600".encode()).decode()),
        _envelope(full, payload=b64(b"\xff\xfe").decode().replace(" ", "")),
    ]


def _timeout_tokens():
    mid = TIMEOUT_MID
    return [
        "60", "0.5", "86400", "86400.0", "86401", "0", "0.0", ".5", "5.",
        "1e3", "1E3", "+5", "-5", "05", "005.5", "86399.999", "86400.5",
        "86400." + mid,                        # exactly at the bound: allowed
        "86400." + mid[:-1] + "6",             # past the bound: denied
        "86400." + mid + "4999",               # past it later: denied
        "9" * 40,                              # six+ significant digits
        "0" * 50 + "1",                        # leading zeros do not count
        "0." + "0" * 330 + "5",                # below half-subnormal: zero
        "0." + "0" * 323 + "3",                # above it: allowed
        "1234567890123456789012345678901234567890.5",
        "1_000", "\uff16\uff10",               # non-ASCII digits: denied
        "6" + "0" * 120,                       # long, value past the cap
        "0" * 100 + "60",                      # long, value 60: allowed
    ]


def _structure_cases(paths):
    good = _envelope(paths["full"])
    quoted = json.dumps(good)
    return [
        _pre_cmd(good),
        ('{"tool_input":{"command":7,"cmd":' + quoted + "}}").encode(),
        ('{"tool_input":{"command":null,"cmd":' + quoted + "}}").encode(),
        ('{"tool_input":{"cmd":' + quoted + '},"tool_input":{"command":"echo no"}}').encode(),
        ('{"tool_input":{"command":"echo no"},"tool_input":{"cmd":' + quoted + "}}").encode(),
        ('{"tool_input":{"command":"echo no","command":' + quoted + "}}").encode(),
        ('{"tool_input":{"command":' + quoted + ',"command":"echo no"}}').encode(),
        ('{"tool_input":{"command":' + quoted + '},"command":"echo no"}').encode(),
        b'{"tool_input":null}',
        b'{"tool_input":[]}',
        b'{"tool_input":{}}',
        ('{"other":{"command":' + quoted + "}}").encode(),
        ('{"extra":{"deep":[[[["x"]]]]},"tool_input":{"command":' + quoted + "}}").encode(),
        b"{}",
        b"[]",
        b"null",
        b'"x"',
        b"",
        b"{",
        b'{"tool_input":{"command":"a",} garbage}',
        b'{"tool_input":{"command":"a",}}',
        b'{"a":1} trailing',
        # Hazard shapes in unwatched positions; deny on either reading.
        b'{"a":1.}',
        b'{"a":.5}',
        b'{"a":1e}',
        b'{"a":1.e5}',
        b'{"a":-.5e-2}',
        b'{"a":+1}',
        b'{"a":1+2}',
        b'{"a":truex}',
        b'{"a":"bad\\ud800"}',
        b'{"a":"bad\\ud800\\u0041"}',
        b'{"a":"ok\\ud83d\\ude00"}',
        b'{"a":"bad\\udc00"}',
        b'{"a":"bad\\u+123"}',
        # Escaped key spellings decode to the watched names.
        b'{"tool\\u005finput":{"command":"echo no"}}',
        b'{"tool_input":{"comm\\u0061nd":"echo no"}}',
        ('{"tool\\u005finput":{"command":' + quoted + "}}").encode(),
        ('{"tool_input":{"comm\\u0061nd":' + quoted + "}}").encode(),
        # Invalid UTF-8 denies, as a failed whole-string read denies.
        b'{\xff}',
        b'{"a":"\xc3"}',
        # Deep unwatched nesting, inside the depth cap and past it.
        (b'{"x":' + b"[" * 100 + b"1" + b"]" * 100 +
         b',"tool_input":{"command":' + quoted.encode() + b"}}"),
        (b'{"x":' + b"[" * 300 + b"1" + b"]" * 300 + b"}"),
    ]


def _window_cases(paths):
    good = _envelope(paths["full"])
    quoted = json.dumps(good)
    cases = []
    fillers = {
        "plain-16380": "a" * 16380,
        "plain-16384": "a" * WINDOW,
        "plain-16385": "a" * (WINDOW + 1),
        "plain-32768": "a" * (2 * WINDOW),
        "plain-100000": "a" * 100000,
        # Multibyte scalars straddling the fill edges, escapes at
        # the edges, long plain runs (the tree's own filler shape).
        "straddle": ("a" * 16380 + "\u00fc\u20ac\U0001f600" + "b" * 40000
                     + "\\u00fc\\n\\\"" + "c" * 40000 + "d" * 16383
                     + "\u00e9" + "e" * 100),
    }
    for name, filler in fillers.items():
        encoded = json.dumps(filler)
        cases.append((f"window-{name}-before",
                      ('{"x":' + encoded + ',"tool_input":{"command":'
                       + quoted + "}}").encode()))
        cases.append((f"window-{name}-watched",
                      ('{"tool_input":{"command":' + encoded + "}}").encode()))
        cases.append((f"window-{name}-key",
                      ("{" + encoded + ':1,"tool_input":{"command":'
                       + quoted + "}}").encode()))
    # A watched string standing after a long unwatched string.
    crossed = (b'{"x":"' + b"a" * 100000 +
               b'","tool_input":{"command":' + quoted.encode() + b"}}")
    cases.append(("window-crossed-100000", crossed))
    # Byte sequences no string may hold (deny; no envelope present).
    for raw in (b'{"x":"\xc3"}', b'{"x":"\xc0\xaf"}',
                b'{"x":"\xed\xa0\x80"}', b'{"x":"\xf4\x90\x80\x80"}',
                b'{"x":"ok\x80"}', b'{"x":"\xe2\x82"}'):
        cases.append(("window-invalid-utf8", raw))
    return cases


def _mutations(paths, rng):
    good = _envelope(paths["full"])
    bases = [
        _pre(good),
        _cur(good),
        _pre("echo no"),
        json.dumps({"tool_input": {"command": good},
                    "extra": {"deep": [[1, 2], {"k": "v"}]},
                    "step": 3}).encode(),
        json.dumps({"tool_input": {"cmd": good, "other": "x" * 64}}).encode(),
    ]
    fragments = [b" ", b"\n", b'"', b"\\", b"\\u0061", b",", b"}", b"{",
                 b'"cmd"', b"123", b"true", b"null", b"\xc3\xa9", b"\xff"]
    cases = []
    for _ in range(90):
        data = bytearray(rng.choice(bases))
        roll = rng.random()
        if roll < 0.35 and data:
            del data[rng.randrange(len(data)):]  # truncate
            data = data[:rng.randrange(len(data) + 1)] if data else data
        elif roll < 0.60 and data:
            at = rng.randrange(len(data))
            data[at:at] = rng.choice(fragments)
        elif roll < 0.85 and data:
            at = rng.randrange(len(data))
            data[at] = rng.randrange(256)
        elif len(data) > 4:
            at = rng.randrange(len(data) - 2)
            data[at], data[at + 1] = data[at + 1], data[at]
        cases.append(bytes(data))
    return cases


def _fuzz(paths, rng):
    good = _envelope(paths["full"])
    quoted = json.dumps(good)
    pool = ['{"tool_input":', '"command"', '"cmd"', ":", quoted, "}", "{",
            ",", " ", "\n", '"a"', "123", "1.5", "true", "null", "[", "]",
            '"\\u0061"', "\u00e9", "\U0001f600", "\\", '"', "x",
            '"tool_input"', '{"command":']
    cases = []
    for _ in range(90):
        cases.append("".join(rng.choice(pool)
                             for _ in range(rng.randrange(3, 15))).encode("utf-8"))
    alphabet = b'{}[]",:0123456789.eE+-truefalsn \\'
    for _ in range(20):
        cases.append(bytes(rng.choice(alphabet)
                           for _ in range(rng.randrange(1, 120))))
    return cases


def generate_corpus(layout):
    """The seeded corpus for `layout`: a list of Case(category,
    payload-bytes), deterministic in the seed and the layout. Every
    payload is below the 16 MiB cap (the cap is another family's
    subject). The family size is len(...) of what this returns."""
    paths = layout_paths(layout)
    rng = random.Random(CORPUS_SEED)
    cases = []
    for command in _directed_commands(paths):
        cases.append(Case("directed", _pre(command)))
    for command in _directed_commands(paths)[:8]:
        cases.append(Case("directed-cursor", _cur(command)))
    cases.append(Case("directed-cursor", _cur("echo no")))
    for token in _timeout_tokens():
        cases.append(Case("timeout-token",
                          _pre(_envelope(paths["full"], timeout=token,
                                         shell="bash"))))
    for payload in _structure_cases(paths):
        cases.append(Case("structure", payload))
    for category, payload in _window_cases(paths):
        cases.append(Case(category, payload))
    for payload in _mutations(paths, rng):
        cases.append(Case("mutation", payload))
    for payload in _fuzz(paths, rng):
        cases.append(Case("fuzz", payload))
    return cases
