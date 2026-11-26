/* Private startup marker. This object is linked only into the two probes. */
#include "crt_probe_frame.h"

static __declspec(noreturn) void fail_probe(PtgCrtFrame *frame)
{
    if (frame != NULL) {
        InterlockedExchange(&frame->phase, PTG_CRT_FAILED);
    }
    ExitProcess(PTG_CRT_FAILURE_EXIT);
}

static int lower_hex(WCHAR digit)
{
    if (digit >= L'0' && digit <= L'9') {
        return digit - L'0';
    }
    if (digit >= L'a' && digit <= L'f') {
        return digit - L'a' + 10;
    }
    return -1;
}

static BOOL parse_credential(const WCHAR *text, HANDLE *mapping, uint8_t *nonce)
{
    uintptr_t handle = 0;
    for (size_t i = 0; i < PTG_CRT_HANDLE_HEX; ++i) {
        const int digit = lower_hex(text[i]);
        if (digit < 0) {
            return FALSE;
        }
        handle = (handle << 4) | (uintptr_t)digit;
    }
    if (handle == 0 || handle == UINTPTR_MAX || text[PTG_CRT_HANDLE_HEX] != L':') {
        return FALSE;
    }
    for (size_t i = 0; i < PTG_CRT_NONCE_BYTES; ++i) {
        const size_t offset = PTG_CRT_HANDLE_HEX + 1 + i * 2;
        const int high = lower_hex(text[offset]);
        const int low = lower_hex(text[offset + 1]);
        if (high < 0 || low < 0) {
            return FALSE;
        }
        nonce[i] = (uint8_t)((high << 4) | low);
    }
    *mapping = (HANDLE)handle;
    return TRUE;
}

static BOOL request_matches(const uint8_t *request, const uint8_t *nonce, DWORD pid, uint64_t epoch)
{
    for (size_t i = 0; i < PTG_CRT_MAGIC_BYTES; ++i) {
        if (request[PTG_CRT_MAGIC_AT + i] != (uint8_t)PTG_CRT_REQUEST_MAGIC[i]) {
            return FALSE;
        }
    }
    if (ptg_crt_read_u32(request + PTG_CRT_VERSION_AT) != PTG_CRT_VERSION
        || ptg_crt_read_u32(request + PTG_CRT_PID_AT) != pid
        || ptg_crt_read_u64(request + PTG_CRT_EPOCH_AT) != epoch) {
        return FALSE;
    }
    for (size_t i = 0; i < PTG_CRT_NONCE_BYTES; ++i) {
        if (request[PTG_CRT_NONCE_AT + i] != nonce[i]) {
            return FALSE;
        }
    }
    return TRUE;
}

static void __cdecl crt_probe_callback(void)
{
    LARGE_INTEGER stamp;
    const BOOL sampled = QueryPerformanceCounter(&stamp);

    WCHAR credential[PTG_CRT_CREDENTIAL_CHARS + 1];
    const DWORD prior_error = GetLastError();
    SetLastError(ERROR_SUCCESS);
    const DWORD length = GetEnvironmentVariableW(PTG_CRT_PROBE_ENV, credential, PTG_CRT_CREDENTIAL_CHARS + 1);
    if (length == 0 && GetLastError() == ERROR_ENVVAR_NOT_FOUND) {
        SetLastError(prior_error);
        return;
    }
    if (!sampled || length != PTG_CRT_CREDENTIAL_CHARS) {
        fail_probe(NULL);
    }

    HANDLE mapping;
    uint8_t nonce[PTG_CRT_NONCE_BYTES];
    if (!parse_credential(credential, &mapping, nonce)) {
        fail_probe(NULL);
    }
    PtgCrtFrame *const frame = MapViewOfFile(mapping, FILE_MAP_WRITE, 0, 0, PTG_CRT_PAGE_BYTES);
    if (frame == NULL || InterlockedCompareExchange(&frame->phase, PTG_CRT_EMPTY, PTG_CRT_EMPTY) != PTG_CRT_EMPTY) {
        fail_probe(NULL);
    }

    const HANDLE self = GetCurrentProcess();
    const DWORD pid = GetCurrentProcessId();
    FILETIME creation, exit_time, kernel, user;
    LARGE_INTEGER frequency;
    if (!GetProcessTimes(self, &creation, &exit_time, &kernel, &user)
        || !QueryPerformanceFrequency(&frequency) || frequency.QuadPart <= 0) {
        fail_probe(NULL);
    }
    const uint64_t epoch = ((uint64_t)creation.dwHighDateTime << 32) | creation.dwLowDateTime;
    if (pid == 0 || epoch == 0 || !request_matches(frame->request, nonce, pid, epoch)) {
        fail_probe(NULL);
    }
    if (InterlockedCompareExchange(&frame->phase, PTG_CRT_WRITING, PTG_CRT_EMPTY) != PTG_CRT_EMPTY) {
        fail_probe(frame);
    }

    uint8_t *const reply = frame->reply;
    for (size_t i = 0; i < PTG_CRT_MAGIC_BYTES; ++i) {
        reply[PTG_CRT_MAGIC_AT + i] = (uint8_t)PTG_CRT_REPLY_MAGIC[i];
    }
    ptg_crt_write_u32(reply + PTG_CRT_VERSION_AT, PTG_CRT_VERSION);
    ptg_crt_write_u32(reply + PTG_CRT_PID_AT, pid);
    ptg_crt_write_u64(reply + PTG_CRT_EPOCH_AT, epoch);
    for (size_t i = 0; i < PTG_CRT_NONCE_BYTES; ++i) {
        reply[PTG_CRT_NONCE_AT + i] = nonce[i];
    }
    ptg_crt_write_u64(reply + PTG_CRT_QPC_AT, (uint64_t)stamp.QuadPart);
    ptg_crt_write_u64(reply + PTG_CRT_QPF_AT, (uint64_t)frequency.QuadPart);
    ptg_crt_write_u32(reply + PTG_CRT_CRC_AT, ptg_crt_reply_crc(reply));
    if (InterlockedCompareExchange(&frame->phase, PTG_CRT_READY, PTG_CRT_WRITING) != PTG_CRT_WRITING) {
        fail_probe(frame);
    }

    TerminateProcess(self, 0);
    /* A successful self-termination never returns. */
    fail_probe(frame);
}

/* Each private AMD64 link explicitly retains /INCLUDE:ptg_crt_probe_initializer. */
#pragma section(".CRT$XCU", read)
__declspec(allocate(".CRT$XCU"))
void (__cdecl *const ptg_crt_probe_initializer)(void) = crt_probe_callback;
