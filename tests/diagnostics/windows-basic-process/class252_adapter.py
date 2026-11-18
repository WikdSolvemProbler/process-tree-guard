"""Explicit class-252 wire decoder and owned native module. No native call on import."""
import ctypes
from ctypes import wintypes as w
import hashlib
import ntpath
import os
from pathlib import Path
import struct
import time

PROVIDER = "nt-system-basic-process-252-v1"
MAX_ROWS = 4096
BUFFER_CAP = 4 * 1024 * 1024
SYSTEM32_SEARCH = 0x800
ROW_WIRE = struct.Struct("<IIQIIIHHI520s4x")

class Row(ctypes.Structure):
    _fields_ = [("pid", ctypes.c_uint32), ("parent_pid", ctypes.c_uint32),
                ("sequence", ctypes.c_uint64), ("record_offset", ctypes.c_uint32),
                ("next_entry_offset", ctypes.c_uint32), ("name_offset", ctypes.c_uint32),
                ("name_length", ctypes.c_uint16), ("name_maximum", ctypes.c_uint16),
                ("name_buffer_present", ctypes.c_uint32), ("name", ctypes.c_uint8 * 520)]

class Evidence(ctypes.Structure):
    _fields_ = [(key, ctypes.c_uint32) for key in (
        "version", "bytes", "information_class", "status", "query_calls", "return_length",
        "row_count", "row_capacity", "buffer_capacity", "clock_error")] + [
        ("ntstatus", ctypes.c_int32), ("parse_index", ctypes.c_uint32), ("upper_ticks", ctypes.c_uint64)
    ] + [(key, ctypes.c_int64) for key in (
        "frequency", "begin_qpc", "after_upper_qpc", "after_decode_qpc")] + [
        ("parse_error", ctypes.c_char * 48)]

class Setup(ctypes.Structure):
    _fields_ = [(key, ctypes.c_uint32) for key in ("bytes", "major", "minor", "build")] + [
        ("ntstatus", ctypes.c_int32), ("system_ntdll_path_verified", ctypes.c_uint32)]


def abi():
    require((ctypes.sizeof(ctypes.c_void_p), ctypes.sizeof(Row), ROW_WIRE.size, Row.name.offset,
             ctypes.sizeof(Evidence), Evidence.upper_ticks.offset, Evidence.parse_error.offset,
             ctypes.sizeof(Setup)) == (8, 560, 560, 36, 136, 48, 88, 24),
            "unsupported class-252 wire ABI")


def fields(value):
    return {key: getattr(value, key) for key, _ in value._fields_}


def capture_used_rows(rows, count):
    require(isinstance(rows, ctypes.Array) and type(rows)._type_ is Row and len(rows) == MAX_ROWS
            and ctypes.sizeof(rows) == ROW_WIRE.size * MAX_ROWS,
            "fixed class-252 row array required")
    require(type(count) is int and 0 <= count <= 2**32 - 1, "native row count type/range")
    used = min(count, MAX_ROWS)  # Bounded partial evidence only; never admission.
    blob = ctypes.string_at(ctypes.addressof(rows), used * ROW_WIRE.size)
    require(len(blob) == used * ROW_WIRE.size, "short class-252 wire copy")
    return tuple(ROW_WIRE.iter_unpack(blob))


def wire_evidence(records):
    return [{"pid": pid, "parent_pid": parent, "sequence_number": sequence, "record_offset": offset,
             "next_entry_offset": link, "name_buffer_offset": pointer, "name_length": length,
             "name_maximum_length": maximum, "name_buffer_present": present,
             "name_utf16le_hex": name[:length].hex()}
            for pid, parent, sequence, offset, link, pointer, length, maximum, present, name in records]


def decode_records(records, raw, returned):
    abi()
    require(type(returned) is int and returned == raw["status"] == raw["ntstatus"] == 0,
            "class-252 query/decoder refused: " + repr(raw))
    require((raw["version"], raw["bytes"], raw["information_class"], raw["row_capacity"],
             raw["buffer_capacity"]) == (1, 136, 252, MAX_ROWS, BUFFER_CAP), "class-252 evidence ABI")
    require(raw["query_calls"] == 1 and raw["clock_error"] == 0 and not raw["parse_error"]
            and raw["parse_index"] == 0 and raw["upper_ticks"] > 0
            and 48 <= raw["return_length"] <= BUFFER_CAP
            and 1 <= len(records) == raw["row_count"] <= MAX_ROWS, "class-252 count/clock/returns")
    chain = [raw[key] for key in ("begin_qpc", "after_upper_qpc", "after_decode_qpc")]
    require(raw["frequency"] > 0 and 0 < chain[0] <= chain[1] <= chain[2], "class-252 native QPC chain")
    output, seen, expected = [], set(), 0
    for index, (pid, parent, _sequence, offset, link, pointer, length, maximum, present, name) in enumerate(records):
        limit = offset + link if link else raw["return_length"]
        require(offset == expected and offset % 8 == 0 and offset + 48 <= limit <= raw["return_length"]
                and bool(link) is (index != len(records) - 1)
                and (not link or link % 8 == 0), "class-252 linked record extent")
        require(pid not in seen and 0 <= length <= 518 and not length % 2 and not maximum % 2
                and length <= maximum and present in (0, 1), "class-252 duplicate/name count")
        if pid == 0:
            require(not any((length, maximum, pointer, present)), "unexpected PID-zero name shape")
            used_end = offset + 48
        else:
            require(length and present and not pointer % 2 and offset + 48 <= pointer < limit
                    and maximum <= limit - pointer, "class-252 selected name storage")
            used_end = pointer + maximum
        require(0 <= limit - used_end < 8 and not any(name[length:]), "class-252 extent/transport tail")
        text = name[:length].decode("utf-16le", "surrogatepass")
        require(not any(ch in text for ch in ("\0", "/", "\\", ":")), "class-252 name is not a basename")
        seen.add(pid)
        output.append({"pid": pid, "parent_pid": parent, "name": text.casefold()})
        expected = limit
    require(expected == raw["return_length"] and 0 in seen, "class-252 endpoint/PID-zero completeness")
    return output


def require(value, message):
    if not value:
        raise ValueError(message)


def pe_imports(raw):
    """Bounded AMD64 PE32+ imports; no executable mapping, fallback or delay imports."""
    require(len(raw) >= 64 and raw[:2] == b"MZ", "not a bounded PE")
    def get(fmt, offset):
        size = struct.calcsize(fmt)
        require(type(offset) is int and 0 <= offset <= len(raw) - size, "PE field exceeds bytes")
        return struct.unpack_from(fmt, raw, offset)
    offset = get("<I", 60)[0]
    require(raw[offset:offset+4] == b"PE\0\0", "PE signature")
    machine, section_count = get("<HH", offset + 4)
    require(machine == 0x8664 and 1 <= section_count <= 96, "AMD64 bounded sections required")
    optional_size = get("<H", offset + 20)[0]
    optional = offset + 24
    require(optional_size >= 240 and get("<H", optional)[0] == 0x20b, "PE32+ optional header")
    require(get("<I", optional + 108)[0] >= 14, "missing PE data directories")
    sections_offset = optional + optional_size
    sections = []
    for index in range(section_count):
        base = sections_offset + index * 40
        virtual_size, address, raw_size, raw_offset = get("<IIII", base + 8)
        require(raw_offset <= len(raw) and raw_size <= len(raw) - raw_offset, "section raw bounds")
        sections.append((address, raw_size, raw_offset))
    def rva(address, size):
        require(0 < size <= len(raw), "RVA size")
        matches = [start + address - base for base, length, start in sections
                   if base <= address and address - base <= length - size]
        require(len(matches) == 1, "unmapped/ambiguous import RVA")
        return matches[0]
    delay_rva, delay_size = get("<II", optional + 112 + 13 * 8)
    require(delay_rva == delay_size == 0, "delay imports are not reviewed")
    import_rva, import_size = get("<II", optional + 112 + 8)
    require(20 <= import_size <= 20 * 129, "bounded import directory required")
    location = rva(import_rva, import_size)
    names = []
    terminated = False
    for index in range(min(import_size // 20, 129)):
        fields = get("<IIIII", location + index * 20)
        if not any(fields):
            terminated = True
            break
        require(len(names) < 128, "too many imports")
        name_start = rva(fields[3], 1)
        name_end = raw.find(b"\0", name_start, min(len(raw), name_start + 261))
        require(name_end > name_start, "unterminated import name")
        require(rva(fields[3], name_end - name_start + 1) == name_start, "import name crosses raw section")
        name = raw[name_start:name_end].decode("ascii", "strict").lower()
        require(ntpath.basename(name) == name and name.endswith(".dll")
                and "/" not in name and "\\" not in name and ":" not in name,
                "non-basename dependency")
        require(name not in names, "duplicate dependency")
        names.append(name)
    require(terminated and names, "unterminated/empty import directory")
    return sorted(names)

def primitive_error(error):
    try:
        message = str(error)
    except BaseException:
        message = "exception text unavailable"
    try:
        winerror = getattr(error, "winerror", None)
    except BaseException:
        winerror = None
    return {"type": type(error).__name__, "message": message,
            "winerror": winerror if type(winerror) is int else None}


def attach_failure(error, evidence, owner=None):
    error.evidence = evidence
    error.quarantine_owner = owner


def retain_capture(rows, native, evidence):
    raw = fields(native)
    raw["parse_error"] = raw["parse_error"].decode("ascii", "backslashreplace")
    evidence["native"] = raw
    records = capture_used_rows(rows, raw["row_count"])
    evidence["raw_utf16_rows"] = wire_evidence(records)
    return records, raw


class BulkSnapshotError(ValueError):
    def __init__(self, message, evidence, quarantine_owner=None):
        super().__init__(message)
        self.evidence = evidence
        self.quarantine_owner = quarantine_owner

class NativeBulk:
    """Owned class-252 code module for one finite observer lifetime, no observation cache.

    DLL/dependency validation and load/unload are explicit setup/cleanup costs.
    Every snapshot allocates/resets its buffer and marshals/validates all rows
    inside that call. No row, identity, path or observation survives on this object.
    """
    def __init__(self, dll_path, dll_sha256, dependency_sha256):
        self.evidence = {"setup_begin_ns": time.perf_counter_ns()}
        self.module = None
        self.closed = False
        self.quarantined = False
        self.release_attempted = False
        self.dll_sha256 = dll_sha256
        self.dependency_sha256 = dict(dependency_sha256)
        try:
            abi()
            self.path = Path(dll_path).resolve(strict=True)
            require(self.path.is_absolute() and self.path.suffix.lower() == ".dll", "absolute DLL required")
            raw = self.path.read_bytes()
            require(hashlib.sha256(raw).hexdigest() == dll_sha256, "DLL pin differs")
            require(pe_imports(raw) == ["kernel32.dll"], "unreviewed class-252 static import set")
            self.dependencies = sorted(self.dependency_sha256)
            require(self.dependencies == ["kernel32.dll", "ntdll.dll"], "unreviewed System32 dependency pins")
            self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
            def bind(name, args, result):
                call = getattr(self.kernel, name)
                call.argtypes, call.restype = args, result
                return call
            system_directory = bind("GetSystemDirectoryW", [w.LPWSTR, w.UINT], w.UINT)
            load = bind("LoadLibraryExW", [w.LPCWSTR, w.HANDLE, w.DWORD], w.HMODULE)
            address = bind("GetProcAddress", [w.HMODULE, ctypes.c_char_p], ctypes.c_void_p)
            self.unload = bind("FreeLibrary", [w.HMODULE], w.BOOL)
            module_name = bind("GetModuleFileNameW", [w.HMODULE, w.LPWSTR, w.DWORD], w.DWORD)
            module_handle = bind("GetModuleHandleW", [w.LPCWSTR], w.HMODULE)
            buffer = ctypes.create_unicode_buffer(32768)
            length = system_directory(buffer, len(buffer))
            require(0 < length < len(buffer), "System32 directory query failed")
            self.system = Path(buffer[:length])
            self.check_code_pins()
            # Absolute primary path and dependencies searched only in System32;
            # no current directory, PATH or writable peer-DLL directory search.
            self.module = load(str(self.path), None, SYSTEM32_SEARCH)
            if not self.module:
                raise ctypes.WinError(ctypes.get_last_error())
            length = module_name(self.module, buffer, len(buffer))
            require(0 < length < len(buffer), "loaded module path query failed")
            loaded_path = os.path.normcase(os.path.abspath(buffer[:length]))
            require(loaded_path == os.path.normcase(str(self.path)), "loaded module location differs")
            self.evidence["loaded_path"] = buffer[:length]
            loaded_dependencies = {}
            for name in self.dependencies:
                borrowed = module_handle(name)
                require(borrowed, "loaded dependency handle unavailable")
                length = module_name(borrowed, buffer, len(buffer))
                require(0 < length < len(buffer), "loaded dependency path query failed")
                require(os.path.normcase(os.path.abspath(buffer[:length]))
                        == os.path.normcase(str(self.system / name)), "dependency loaded outside System32")
                loaded_dependencies[name] = buffer[:length]
            self.evidence["loaded_dependency_paths"] = loaded_dependencies
            setup_pointer = address(self.module, b"ptg_class252_setup")
            if not setup_pointer:
                raise ctypes.WinError(ctypes.get_last_error())
            setup = ctypes.CFUNCTYPE(ctypes.c_uint32, ctypes.POINTER(Setup))(setup_pointer)
            native_setup = Setup()
            setup_status = int(setup(ctypes.byref(native_setup)))
            self.evidence["native_provider"] = PROVIDER
            self.evidence["native_setup"] = fields(native_setup)
            self.evidence["system32_pin_basis"] = "Loaded paths and on-disk digests checked separately"
            require(setup_status == 0 and fields(native_setup) == {
                "bytes": 24, "major": 10, "minor": 0, "build": 26300,
                "ntstatus": 0, "system_ntdll_path_verified": 1,
            }, "unqualified running Windows build, ABI or System32 ntdll")
            pointer = address(self.module, b"ptg_class252_capture")
            if not pointer:
                raise ctypes.WinError(ctypes.get_last_error())
            self.call = ctypes.CFUNCTYPE(ctypes.c_uint32, ctypes.POINTER(Row), ctypes.c_uint32,
                                        ctypes.POINTER(Evidence), ctypes.POINTER(ctypes.c_uint8),
                                        ctypes.c_uint32)(pointer)
            self.check_code_pins()
            self.evidence.update(dll_sha256=dll_sha256, dependency_sha256=self.dependency_sha256,
                                 setup_end_ns=time.perf_counter_ns())
        except BaseException as error:
            self.evidence["setup_error"] = primitive_error(error)
            self.release_owned_module()
            self.closed = True
            try:
                self.evidence["setup_end_ns"] = time.perf_counter_ns()
            except BaseException as clock_error:
                self.evidence["setup_end_clock_error"] = primitive_error(clock_error)
            owner = self if self.quarantined else None
            if not isinstance(error, Exception):
                attach_failure(error, self.evidence, owner)
                raise
            raise BulkSnapshotError(self.evidence["setup_error"]["message"], self.evidence, owner) from error

    def release_owned_module(self):
        if self.module is None:
            return None
        if self.release_attempted:
            return RuntimeError("class-252 module release was already attempted")
        self.release_attempted = True
        self.evidence["release_attempted"] = True
        try:
            if not self.unload(self.module):
                raise ctypes.WinError(ctypes.get_last_error())
        except BaseException as error:
            self.quarantined = True
            self.evidence["quarantined_module_handle"] = self.module
            self.evidence["release_error"] = primitive_error(error)
            return error
        self.module = None
        return None

    def check_code_pins(self):
        require(hashlib.sha256(self.path.read_bytes()).hexdigest() == self.dll_sha256, "DLL pin differs")
        for name in self.dependencies:
            require(hashlib.sha256((self.system / name).read_bytes()).hexdigest()
                    == self.dependency_sha256[name], "System32 dependency pin differs")

    def snapshot(self):
        evidence = {"provider": PROVIDER, "begin_ns": time.perf_counter_ns(),
                    "scope": "one class-252 query; all decoding and evidence inside the census"}
        rows = native = None
        try:
            require(not self.closed and self.module, "class-252 module is not owned/open")
            rows = (Row * MAX_ROWS)()
            buffer = (ctypes.c_uint8 * BUFFER_CAP)()
            native = Evidence()
            evidence["native_begin_ns"] = time.perf_counter_ns()
            evidence["native_call_returned"] = False
            returned = int(self.call(rows, MAX_ROWS, ctypes.byref(native), buffer, BUFFER_CAP))
            evidence["native_call_returned"] = True
            evidence["native_end_ns"] = time.perf_counter_ns()
            records, raw = retain_capture(rows, native, evidence)
            output = decode_records(records, raw, returned)
            evidence["end_ns"] = time.perf_counter_ns()
            return output, raw["upper_ticks"], evidence
        except BaseException as error:
            evidence["error"] = primitive_error(error)
            if "native_begin_ns" in evidence and "raw_utf16_rows" not in evidence:
                try:
                    # Recover bytes already written by the single attempt. This
                    # never calls the provider again or promotes a partial read.
                    retain_capture(rows, native, evidence)
                except BaseException as receipt_error:
                    evidence["native_receipt_error"] = primitive_error(receipt_error)
            try:
                evidence["end_ns"] = time.perf_counter_ns()
            except BaseException as clock_error:
                evidence["end_clock_error"] = primitive_error(clock_error)
            if not isinstance(error, Exception):
                attach_failure(error, evidence)
                raise
            raise BulkSnapshotError(evidence["error"]["message"], evidence) from error

    def close(self):
        if self.closed:
            raise BulkSnapshotError("native module already closed", self.evidence,
                                    self if self.quarantined else None)
        failures = []
        try:
            self.evidence["cleanup_begin_ns"] = time.perf_counter_ns()
        except BaseException as clock_error:
            failures.append(("cleanup_begin_clock", clock_error))
        try:
            self.check_code_pins()
        except BaseException as pin_error:
            failures.append(("code_pins", pin_error))
        release_error = self.release_owned_module()
        if release_error is not None:
            failures.append(("module_release", release_error))
        self.closed = True
        try:
            self.evidence["cleanup_end_ns"] = time.perf_counter_ns()
        except BaseException as clock_error:
            failures.append(("cleanup_end_clock", clock_error))
        records = [{"step": step, **primitive_error(error)} for step, error in failures]
        self.evidence["cleanup_errors"] = records
        if failures:
            owner = self if self.quarantined else None
            primary = failures[0][1]
            if not isinstance(primary, Exception):
                attach_failure(primary, self.evidence, owner)
                raise primary
            raise BulkSnapshotError(records[0]["message"], self.evidence, owner) from primary

    def __enter__(self):
        require(not self.closed and self.module, "module setup failed")
        return self

    def __exit__(self, kind, error, traceback):
        try:
            self.close()
        except BaseException as cleanup:
            if error is None:
                raise
            error.module_cleanup_evidence = self.evidence
            error.module_cleanup_error = primitive_error(cleanup)
            if self.quarantined:
                error.quarantine_owner = self
        return False
