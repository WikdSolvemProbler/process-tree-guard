/* Separate SystemBasicProcessInformation provider. No Toolhelp fallback. */
#define BASIC_PROBE_PARSER_ONLY 1
#include "basic_process_probe.c"

typedef NTSTATUS (NTAPI *QuerySystem)(SYSTEM_INFORMATION_CLASS, PVOID, ULONG, PULONG);
typedef NTSTATUS (NTAPI *QueryVersion)(PRTL_OSVERSIONINFOW);

typedef struct {
    uint32_t pid, parent_pid;
    uint64_t sequence;
    uint32_t record_offset, next_entry_offset, name_offset;
    uint16_t name_length, name_maximum;
    uint32_t name_buffer_present;
    uint8_t name[520];
} WireRow;

typedef struct {
    uint32_t version, bytes, information_class, status;
    uint32_t query_calls, return_length, row_count, row_capacity, buffer_capacity, clock_error;
    int32_t ntstatus;
    uint32_t parse_index;
    uint64_t upper_ticks;
    int64_t frequency, begin_qpc, after_upper_qpc, after_decode_qpc;
    char parse_error[48];
} WireEvidence;

typedef struct {
    uint32_t bytes, major, minor, build;
    int32_t ntstatus;
    uint32_t system_ntdll_path_verified;
} WireSetup;

_Static_assert(sizeof(WireRow) == 560 && offsetof(WireRow, name) == 36, "class252 row ABI");
_Static_assert(sizeof(WireEvidence) == 136 && offsetof(WireEvidence, upper_ticks) == 48
               && offsetof(WireEvidence, parse_error) == 88, "class252 evidence ABI");
_Static_assert(sizeof(WireSetup) == 24, "class252 setup ABI");
_Static_assert(sizeof(QuerySystem) == sizeof(FARPROC) && sizeof(QueryVersion) == sizeof(FARPROC),
               "native export ABI");

static QuerySystem query;

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

__declspec(dllexport) uint32_t ptg_class252_setup(WireSetup *wire)
{
    if (!wire)
        return 1;
    *wire = (WireSetup){0};
    wire->bytes = (uint32_t)sizeof(*wire);
    HMODULE ntdll;
    if (query || !system_ntdll(&ntdll))
        return 1;
    wire->system_ntdll_path_verified = 1;
    FARPROC provider = GetProcAddress(ntdll, "NtQuerySystemInformation");
    FARPROC rtl_version = GetProcAddress(ntdll, "RtlGetVersion");
    if (!provider || !rtl_version)
        return 1;
    QueryVersion version;
    memcpy(&version, &rtl_version, sizeof(version));
    RTL_OSVERSIONINFOW os = {0};
    os.dwOSVersionInfoSize = (ULONG)sizeof(os);
    wire->ntstatus = version(&os);
    wire->major = os.dwMajorVersion;
    wire->minor = os.dwMinorVersion;
    wire->build = os.dwBuildNumber;
    if (wire->ntstatus || wire->major != 10 || wire->minor != 0 || wire->build != 26300)
        return 1;
    memcpy(&query, &provider, sizeof(query));
    return 0;
}

__declspec(dllexport) uint32_t ptg_class252_capture(WireRow *rows, uint32_t row_capacity,
                                                  WireEvidence *wire, uint8_t *buffer,
                                                  uint32_t buffer_capacity)
{
    if (!wire)
        return 1;
    *wire = (WireEvidence){0};
    wire->version = 1;
    wire->bytes = (uint32_t)sizeof(*wire);
    wire->information_class = SystemBasicProcessInformation;
    wire->row_capacity = row_capacity;
    wire->buffer_capacity = buffer_capacity;
    if (!query || !rows || row_capacity != ROW_CAP || !buffer || buffer_capacity != BUFFER_CAP
            || (uintptr_t)buffer % _Alignof(SYSTEM_BASICPROCESS_INFORMATION))
        return wire->status = 1;

    LARGE_INTEGER freq = {0}, begin = {0}, upper_end = {0}, decoded_end = {0};
    FILETIME upper = {0};
    ULONG returned = 0;
    if (!QueryPerformanceFrequency(&freq) || freq.QuadPart <= 0
            || !QueryPerformanceCounter(&begin) || begin.QuadPart <= 0) {
        wire->clock_error = 1;
        return wire->status = 2;
    }
    const NTSTATUS status = query(SystemBasicProcessInformation, buffer, buffer_capacity, &returned);
    if (status == 0)
        GetSystemTimePreciseAsFileTime(&upper);
    const BOOL upper_clock_ok = QueryPerformanceCounter(&upper_end);
    wire->query_calls = 1;
    wire->ntstatus = status;
    wire->return_length = returned;
    wire->upper_ticks = ((uint64_t)upper.dwHighDateTime << 32) | upper.dwLowDateTime;
    wire->frequency = freq.QuadPart;
    wire->begin_qpc = begin.QuadPart;
    wire->after_upper_qpc = upper_end.QuadPart;
    if (!upper_clock_ok || upper_end.QuadPart < begin.QuadPart)
        wire->clock_error = 2;
    if (status)
        return wire->status = 3;
    if (wire->clock_error || !wire->upper_ticks)
        return wire->status = 2;

    BasicRow decoded[ROW_CAP];
    size_t count = 0;
    const ParseResult parsed = decode_rows(buffer, returned, buffer_capacity, decoded, ROW_CAP, &count);
    wire->row_count = (uint32_t)count;
    for (size_t index = 0; index < count; ++index) {
        const BasicRow *item = &decoded[index];
        WireRow *row = &rows[index];
        *row = (WireRow){0};
        row->pid = item->pid;
        row->parent_pid = item->parent;
        row->sequence = item->sequence;
        row->record_offset = (uint32_t)item->record_offset;
        row->next_entry_offset = item->next_offset;
        row->name_offset = (uint32_t)item->name_offset;
        row->name_length = item->length;
        row->name_maximum = item->maximum;
        row->name_buffer_present = item->buffer_present != FALSE;
        if (item->length)
            memcpy(row->name, buffer + item->name_offset, item->length);
    }
    if (!QueryPerformanceCounter(&decoded_end) || decoded_end.QuadPart < upper_end.QuadPart)
        wire->clock_error = 3;
    wire->after_decode_qpc = decoded_end.QuadPart;
    if (parsed.reason) {
        wire->parse_index = (uint32_t)parsed.index;
        const size_t length = strlen(parsed.reason);
        if (length < sizeof(wire->parse_error))
            memcpy(wire->parse_error, parsed.reason, length);
        return wire->status = 4;
    }
    if (wire->clock_error)
        return wire->status = 2;
    return 0;
}
