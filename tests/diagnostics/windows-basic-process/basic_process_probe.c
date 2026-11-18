/* Source-only standalone feasibility probe; no process handles or child work. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <winternl.h>
#include <inttypes.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <wchar.h>

#ifndef _M_AMD64
#error This source is pinned to the AMD64 SDK layout.
#endif

#define BUFFER_CAP ((size_t)4 * 1024 * 1024)
#define ROW_CAP 4096u
#define NAME_UNITS_CAP 259u
#define HEAD_BYTES sizeof(SYSTEM_BASICPROCESS_INFORMATION)

_Static_assert(sizeof(void *) == 8 && sizeof(WCHAR) == 2, "reviewed AMD64/UTF16 ABI");
_Static_assert(sizeof(NTSTATUS) == 4, "32-bit NTSTATUS");
_Static_assert(SystemBasicProcessInformation == 252, "reviewed SDK class");
_Static_assert(HEAD_BYTES == 48
               && offsetof(SYSTEM_BASICPROCESS_INFORMATION, UniqueProcessId) == 8
               && offsetof(SYSTEM_BASICPROCESS_INFORMATION, InheritedFromUniqueProcessId) == 16
               && offsetof(SYSTEM_BASICPROCESS_INFORMATION, SequenceNumber) == 24
               && offsetof(SYSTEM_BASICPROCESS_INFORMATION, ImageName) == 32
               && offsetof(UNICODE_STRING, Buffer) == 8, "reviewed AMD64 native record");

typedef struct {
    size_t record_offset;
    ULONG next_offset;
    uint32_t pid;
    uint32_t parent;
    uint64_t sequence;
    size_t name_offset;
    USHORT length;
    USHORT maximum;
    BOOL buffer_present;
} BasicRow;

typedef struct {
    const char *reason;
    size_t index;
} ParseResult;

/* Source pointers are returned within the same bounded NT buffer. Never
   dereference them directly. Only validated relative offsets leave this pass. */
static ParseResult decode_rows(const uint8_t *bytes, size_t returned, size_t capacity,
                               BasicRow *rows, size_t row_cap, size_t *count)
{
    size_t offset = 0;
    *count = 0;
    if (returned < HEAD_BYTES || returned > capacity)
        return (ParseResult){"returned_length", 0};
    const uintptr_t base = (uintptr_t)bytes;
    if (returned > UINTPTR_MAX - base)
        return (ParseResult){"buffer_address_range", 0};

    for (size_t index = 0; index < row_cap; ++index) {
        if (offset > returned - HEAD_BYTES || offset % _Alignof(SYSTEM_BASICPROCESS_INFORMATION))
            return (ParseResult){"record_offset", index};
        SYSTEM_BASICPROCESS_INFORMATION item;
        memcpy(&item, bytes + offset, HEAD_BYTES);
        const size_t next = item.NextEntryOffset;
        if (next && (next < HEAD_BYTES || next % _Alignof(SYSTEM_BASICPROCESS_INFORMATION)
                     || next > returned - offset - HEAD_BYTES))
            return (ParseResult){"next_entry_offset", index};
        const size_t extent = next ? next : returned - offset;
        const uintptr_t pid = (uintptr_t)item.UniqueProcessId;
        const uintptr_t parent = (uintptr_t)item.InheritedFromUniqueProcessId;
        if (pid > UINT32_MAX || parent > UINT32_MAX)
            return (ParseResult){"pid_width", index};
        for (size_t previous = 0; previous < index; ++previous) {
            if (rows[previous].pid == (uint32_t)pid)
                return (ParseResult){"duplicate_pid", index};
        }

        const USHORT length = item.ImageName.Length;
        const USHORT maximum = item.ImageName.MaximumLength;
        const uintptr_t pointer = (uintptr_t)item.ImageName.Buffer;
        if ((length % 2u) || (maximum % 2u) || length > maximum || length / 2u > NAME_UNITS_CAP)
            return (ParseResult){"utf16_count", index};
        /* Only the native PID-zero pseudo-row may be unnamed.
           Keep that raw row (including its parent, sequence and empty name). */
        if (!length && pid != 0)
            return (ParseResult){"unexpected_empty_name", index};
        size_t name_offset = 0;
        if (pointer) {
            if ((pointer % _Alignof(WCHAR)) || pointer < base || pointer - base >= returned)
                return (ParseResult){"utf16_pointer", index};
            name_offset = pointer - base;
            if (name_offset < offset + HEAD_BYTES || name_offset >= offset + extent
                    || maximum > (offset + extent) - name_offset)
                return (ParseResult){"utf16_record_bounds", index};
        } else if (length || maximum) {
            return (ParseResult){"null_utf16_buffer", index};
        }
        /* No unexplained trailing extent or early list terminator. Permit
           only alignment padding after the validated native name storage. */
        const size_t payload = pointer ? name_offset - offset + maximum : HEAD_BYTES;
        if (extent - payload >= _Alignof(SYSTEM_BASICPROCESS_INFORMATION))
            return (ParseResult){"unexplained_record_extent", index};
        for (size_t unit = 0; unit < length; unit += 2u) {
            if (bytes[name_offset + unit] == 0 && bytes[name_offset + unit + 1] == 0)
                return (ParseResult){"embedded_utf16_nul", index};
        }
        rows[index] = (BasicRow){offset, item.NextEntryOffset, (uint32_t)pid, (uint32_t)parent,
                                 (uint64_t)item.SequenceNumber, name_offset, length, maximum,
                                 pointer != 0};
        *count = index + 1;
        if (!next)
            return (ParseResult){NULL, index};
        offset += next;
    }
    return (ParseResult){"row_capacity_before_last_entry", row_cap};
}

#ifndef BASIC_PROBE_PARSER_ONLY

typedef NTSTATUS (NTAPI *QuerySystem)(SYSTEM_INFORMATION_CLASS, PVOID, ULONG, PULONG);
typedef NTSTATUS (NTAPI *QueryVersion)(PRTL_OSVERSIONINFOW);

_Static_assert(sizeof(QuerySystem) == sizeof(FARPROC) && sizeof(QueryVersion) == sizeof(FARPROC),
               "reviewed Windows export pointer ABI");

__declspec(align(16)) static uint8_t query_buffer[BUFFER_CAP];
static BasicRow decoded_rows[ROW_CAP];

static BOOL system_ntdll(HMODULE *handle)
{
    WCHAR directory[MAX_PATH];
    WCHAR actual[MAX_PATH];
    const UINT used = GetSystemDirectoryW(directory, (UINT)ARRAYSIZE(directory));
    if (!used || used >= ARRAYSIZE(directory)
            || used + ARRAYSIZE(L"\\ntdll.dll") > ARRAYSIZE(directory))
        return FALSE;
    memcpy(directory + used, L"\\ntdll.dll", sizeof(L"\\ntdll.dll"));
    /* ntdll is preloaded by Windows; borrow its process-lifetime handle. */
    *handle = GetModuleHandleW(L"ntdll.dll");
    if (!*handle)
        return FALSE;
    const DWORD actual_length = GetModuleFileNameW(*handle, actual, (DWORD)ARRAYSIZE(actual));
    return actual_length != 0 && actual_length < ARRAYSIZE(actual)
        && CompareStringOrdinal(actual, -1, directory, -1, TRUE) == CSTR_EQUAL;
}

static int finish(FILE *stream, const char *stage, BOOL complete, int code)
{
    if (ferror(stream)) {
        stage = "evidence_write";
        complete = FALSE;
        code = 9;
    }
    const int written = fprintf(stream, "{\"kind\":\"terminal\",\"stage\":\"%s\","
                    "\"complete\":%s,\"exit\":%d}\n", stage, complete ? "true" : "false", code);
    const int flush = fflush(stream);
    const int stream_error = ferror(stream);
    const int close = fclose(stream);
    return written < 0 || flush || stream_error || close ? 9 : code;
}

int wmain(int argc, wchar_t **argv)
{
    if (argc != 2)
        return 2;
    FILE *out = NULL;
    if (_wfopen_s(&out, argv[1], L"wbxc") || !out)
        return 3;

    HMODULE ntdll;
    if (!system_ntdll(&ntdll))
        return finish(out, "loaded_system_ntdll", FALSE, 4);
    const FARPROC native_export = GetProcAddress(ntdll, "NtQuerySystemInformation");
    const FARPROC version_export = GetProcAddress(ntdll, "RtlGetVersion");
    if (!native_export || !version_export)
        return finish(out, "required_ntdll_exports", FALSE, 4);
    QuerySystem query;
    QueryVersion version;
    memcpy(&query, &native_export, sizeof(query));
    memcpy(&version, &version_export, sizeof(version));

    RTL_OSVERSIONINFOW os = {0};
    os.dwOSVersionInfoSize = (ULONG)sizeof(os);
    const NTSTATUS version_status = version(&os);
    fprintf(out, "{\"kind\":\"setup\",\"system_ntdll_path_verified\":true,"
                 "\"version_status\":%ld,\"major\":%lu,\"minor\":%lu,\"build\":%lu,"
                 "\"class\":%d,\"pointer_bytes\":%zu,\"record_bytes\":%zu,"
                 "\"abi_offsets\":[8,16,24,32,40],\"capacity_bytes\":%zu,"
                 "\"row_capacity\":%u}\n",
            (long)version_status, (unsigned long)os.dwMajorVersion, (unsigned long)os.dwMinorVersion,
            (unsigned long)os.dwBuildNumber, (int)SystemBasicProcessInformation, sizeof(void *),
            HEAD_BYTES, BUFFER_CAP, ROW_CAP);
    if (version_status != 0 || os.dwMajorVersion != 10 || os.dwMinorVersion != 0
            || os.dwBuildNumber < 26100u)
        return finish(out, "unsupported_running_build", FALSE, 5);
    if (ferror(out))
        return finish(out, "setup_evidence_write", FALSE, 9);

    LARGE_INTEGER frequency = {0}, begin = {0}, end = {0};
    ULONG returned = 0;
    FILETIME upper = {0};
    if (!QueryPerformanceFrequency(&frequency) || frequency.QuadPart <= 0
            || !QueryPerformanceCounter(&begin) || begin.QuadPart <= 0)
        return finish(out, "initial_qpc", FALSE, 6);
    const NTSTATUS status = query(SystemBasicProcessInformation, query_buffer,
                                  (ULONG)sizeof(query_buffer), &returned);
    if (status == 0)
        GetSystemTimePreciseAsFileTime(&upper);
    const BOOL end_ok = QueryPerformanceCounter(&end);
    const uint64_t upper_ticks = ((uint64_t)upper.dwHighDateTime << 32) | upper.dwLowDateTime;
    fprintf(out, "{\"kind\":\"native\",\"query_calls\":1,\"ntstatus\":%ld,"
                 "\"return_length\":%lu,\"upper_ticks\":%" PRIu64 ","
                 "\"qpc_frequency\":%" PRId64 ",\"begin_qpc\":%" PRId64 ","
                 "\"end_qpc\":%" PRId64 ",\"end_qpc_return\":%d}\n",
            (long)status, (unsigned long)returned, upper_ticks, (int64_t)frequency.QuadPart,
            (int64_t)begin.QuadPart, (int64_t)end.QuadPart, (int)end_ok);
    if (status != 0)
        return finish(out, "native_status", FALSE, 7);
    if (!end_ok || end.QuadPart < begin.QuadPart || !upper_ticks)
        return finish(out, "native_clock_or_upper", FALSE, 6);

    size_t count = 0;
    const ParseResult parsed = decode_rows(query_buffer, returned, sizeof(query_buffer),
                                          decoded_rows, ROW_CAP, &count);
    for (size_t index = 0; index < count; ++index) {
        const BasicRow *row = &decoded_rows[index];
        fprintf(out, "{\"kind\":\"row\",\"index\":%zu,\"record_offset\":%zu,"
                     "\"next_entry_offset\":%lu,\"pid\":%" PRIu32 ",\"parent_pid\":%" PRIu32 ","
                     "\"sequence_number\":%" PRIu64 ",\"name_buffer_present\":%s,"
                     "\"name_buffer_offset\":%zu,\"name_length\":%u,\"name_maximum_length\":%u,"
                     "\"name_utf16le_hex\":\"", index, row->record_offset,
                (unsigned long)row->next_offset, row->pid, row->parent, row->sequence,
                row->buffer_present ? "true" : "false", row->name_offset, (unsigned)row->length,
                (unsigned)row->maximum);
        for (size_t byte = 0; byte < row->length; ++byte)
            fprintf(out, "%02x", (unsigned)query_buffer[row->name_offset + byte]);
        fputs("\"}\n", out);
    }
    if (parsed.reason) {
        fprintf(out, "{\"kind\":\"parse_error\",\"reason\":\"%s\",\"index\":%zu,"
                     "\"valid_prefix_rows\":%zu}\n", parsed.reason, parsed.index, count);
        return finish(out, "decode", FALSE, 8);
    }
    fprintf(out, "{\"kind\":\"decoded\",\"rows\":%zu,\"last_entry_seen\":true}\n", count);
    return finish(out, "complete", TRUE, 0);
}
#endif
