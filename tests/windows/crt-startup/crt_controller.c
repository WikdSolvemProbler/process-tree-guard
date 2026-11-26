/* Private one-use controller for PROTOCOL-003. No payload arguments are accepted. */
#include "crt_probe_frame.h"

#include <bcrypt.h>
#include <fcntl.h>
#include <inttypes.h>
#include <io.h>
#include <stdio.h>
#include <string.h>
#include <wchar.h>

enum { SHA_BYTES = 32, PIPE_BYTES = 4096, SLOTS = 80 };
static const char schedule_sha[] = "8744072cb85aeec6208a88a03c05962273874f441601ab6e71bc95e1aa5bd2d3";
static const char pairs[SLOTS / 2][3] = {
    "BB", "BA", "AB", "AA", "BA",
    "AB", "AA", "AB", "BB", "AB",
    "AB", "BB", "AA", "BA", "BA",
    "BA", "AA", "BB", "AB", "BA",
    "AB", "AB", "AA", "BB", "BA",
    "AA", "AB", "AB", "BB", "BA",
    "BB", "AB", "AB", "BA", "AA",
    "BA", "BA", "AA", "BA", "BB"
};
static const WCHAR command_line[] = L"\"ptg-crt-probe\" --help";

typedef struct {
    const WCHAR *path;
    HANDLE file;
    FILE_ID_INFO identity;
    uint8_t sha[SHA_BYTES], path_sha[SHA_BYTES];
} Image;

typedef struct {
    HANDLE reader, writer;
    BOOL peek_called, peek_ok, read_called, read_ok;
    DWORD peek_error, available, read_error, bytes;
    uint8_t data[PIPE_BYTES];
} Output;

typedef struct {
    const char *stage;
    DWORD error;
    NTSTATUS rng_status;
    BOOL created, member_checked, member, has_epoch, image_joined, terminal_identity;
    DWORD pid, original_pid, terminal_pid;
    uint64_t epoch, terminal_epoch;
    ULONG_PTR credential_handle;
    WCHAR environment[2 * MAX_PATH];
    DWORD environment_bytes;
    uint8_t environment_sha[SHA_BYTES], request_sha[SHA_BYTES], reply_sha[SHA_BYTES];
    uint8_t request[PTG_CRT_REQUEST_BYTES];
    int64_t before_resume, after_resume, cutoff, before_wait, after_wait, after_job, settled;
    BOOL resumed, waited, exit_read, job_read, frame_read, frame_hashed, environment_hashed;
    BOOL pre_resume_polled, release_failed;
    DWORD resume_count, resume_error, wait_ms, status, wait_error, exit_code;
    DWORD pre_resume_status, pre_resume_error, release_error;
    DWORD active, total;
    PtgCrtFrame raw_frame;
    LONG phase;
    BOOL assisted, kill_ok, direct_kill, direct_kill_ok, cleanup_waited, cleanup_job_read;
    DWORD kill_error, direct_kill_error, cleanup_wait_ms, cleanup_wait, cleanup_wait_error;
    DWORD cleanup_clock_error, cleanup_job_error, cleanup_active;
    Output out, err;
} Slot;

typedef struct {
    BCRYPT_ALG_HANDLE sha;
    int64_t frequency, start, aggregate_end, slot_ticks;
    const WCHAR *cwd;
    WCHAR windows[MAX_PATH];
    unsigned nonce_count;
    uint8_t nonces[SLOTS][PTG_CRT_NONCE_BYTES];
    FILE *journal;
} Run;

static BOOL absolute_path(const WCHAR *path)
{
    const WCHAR c = path[0];
    return ((c >= L'A' && c <= L'Z') || (c >= L'a' && c <= L'z'))
        && path[1] == L':' && path[2] == L'\\';
}

static BOOL close_handle(HANDLE *handle)
{
    const BOOL ok = *handle == NULL || *handle == INVALID_HANDLE_VALUE || CloseHandle(*handle);
    *handle = NULL;
    return ok;
}

static void close_slot_handle(HANDLE *handle, Slot *row)
{
    if (!close_handle(handle) && !row->release_failed) {
        row->release_error = GetLastError();
        row->release_failed = TRUE;
    }
}

static BOOL stamp(int64_t *out, DWORD *error)
{
    LARGE_INTEGER now;
    if (!QueryPerformanceCounter(&now)) {
        *error = GetLastError();
        return FALSE;
    }
    *out = now.QuadPart;
    return now.QuadPart >= 0;
}

static uint64_t filetime(FILETIME time)
{
    return ((uint64_t)time.dwHighDateTime << 32) | time.dwLowDateTime;
}

static int hex_digit(WCHAR c)
{
    if (c >= L'0' && c <= L'9') return c - L'0';
    if (c >= L'a' && c <= L'f') return c - L'a' + 10;
    return -1;
}

static BOOL digest_argument(const WCHAR *text, uint8_t *sha)
{
    if (wcslen(text) != SHA_BYTES * 2) return FALSE;
    for (size_t i = 0; i < SHA_BYTES; ++i) {
        const int a = hex_digit(text[i * 2]), b = hex_digit(text[i * 2 + 1]);
        if (a < 0 || b < 0) return FALSE;
        sha[i] = (uint8_t)((a << 4) | b);
    }
    return TRUE;
}

static BOOL hash_bytes(Run *run, const void *bytes, ULONG size, uint8_t *digest)
{
    return BCryptHash(run->sha, NULL, 0, (PUCHAR)bytes, size, digest, SHA_BYTES) == 0;
}

static BOOL hash_file(Run *run, HANDLE file, uint8_t *digest)
{
    BCRYPT_HASH_HANDLE hash = NULL;
    uint8_t bytes[16384];
    BOOL ok = FALSE;
    if (BCryptCreateHash(run->sha, &hash, NULL, 0, NULL, 0, 0) != 0) return FALSE;
    for (;;) {
        DWORD read;
        if (!ReadFile(file, bytes, (DWORD)sizeof(bytes), &read, NULL)) break;
        if (read == 0) {
            ok = BCryptFinishHash(hash, digest, SHA_BYTES, 0) == 0;
            break;
        }
        if (BCryptHashData(hash, bytes, read, 0) != 0) break;
    }
    BCryptDestroyHash(hash);
    return ok;
}

static BOOL regular_image(HANDLE file, FILE_ID_INFO *identity)
{
    FILE_ATTRIBUTE_TAG_INFO attrs;
    return GetFileType(file) == FILE_TYPE_DISK
        && GetFileInformationByHandleEx(file, FileAttributeTagInfo, &attrs, (DWORD)sizeof(attrs))
        && !(attrs.FileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT))
        && GetFileInformationByHandleEx(file, FileIdInfo, identity, (DWORD)sizeof(*identity));
}

static BOOL same_image(const FILE_ID_INFO *a, const FILE_ID_INFO *b)
{
    return a->VolumeSerialNumber == b->VolumeSerialNumber
        && memcmp(a->FileId.Identifier, b->FileId.Identifier, sizeof(a->FileId.Identifier)) == 0;
}

static BOOL pin_image(Run *run, Image *image, const WCHAR *path, const WCHAR *sha)
{
    uint8_t observed[SHA_BYTES];
    const size_t chars = wcslen(path) + 1;
    if (!absolute_path(path) || chars > MAX_PATH || !digest_argument(sha, image->sha)
        || !hash_bytes(run, path, (ULONG)(chars * sizeof(WCHAR)), image->path_sha)) return FALSE;
    image->path = path;
    image->file = CreateFileW(path, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING,
                              FILE_FLAG_OPEN_REPARSE_POINT, NULL);
    return image->file != INVALID_HANDLE_VALUE && regular_image(image->file, &image->identity)
        && hash_file(run, image->file, observed) && memcmp(observed, image->sha, SHA_BYTES) == 0;
}

static BOOL join_original_image(HANDLE process, const Image *image)
{
    WCHAR name[MAX_PATH];
    DWORD chars = MAX_PATH;
    FILE_ID_INFO identity;
    if (!QueryFullProcessImageNameW(process, 0, name, &chars) || chars == 0 || chars >= MAX_PATH) return FALSE;
    HANDLE file = CreateFileW(name, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING,
                              FILE_FLAG_OPEN_REPARSE_POINT, NULL);
    BOOL same = file != INVALID_HANDLE_VALUE && regular_image(file, &identity)
        && same_image(&image->identity, &identity);
    return close_handle(&file) && same;
}

static void make_environment(const Run *run, Slot *row, HANDLE mapping, const uint8_t *nonce)
{
    static const WCHAR root[] = L"SystemRoot=";
    static const WCHAR key[] = PTG_CRT_PROBE_ENV L"=";
    static const WCHAR hex[] = L"0123456789abcdef";
    WCHAR *p = row->environment;
    memcpy(p, key, sizeof(key) - sizeof(WCHAR));
    p += _countof(key) - 1;
    row->credential_handle = (ULONG_PTR)mapping;
    for (unsigned i = PTG_CRT_HANDLE_HEX; i > 0; --i) {
        *p++ = hex[(row->credential_handle >> (4 * (i - 1))) & 15];
    }
    *p++ = L':';
    for (size_t i = 0; i < PTG_CRT_NONCE_BYTES; ++i) {
        *p++ = hex[nonce[i] >> 4];
        *p++ = hex[nonce[i] & 15];
    }
    *p++ = L'\0';
    memcpy(p, root, sizeof(root) - sizeof(WCHAR));
    p += _countof(root) - 1;
    const size_t chars = wcslen(run->windows) + 1;
    memcpy(p, run->windows, chars * sizeof(WCHAR));
    p += chars;
    *p++ = L'\0';
    row->environment_bytes = (DWORD)((size_t)(p - row->environment) * sizeof(WCHAR));
}

static BOOL prepare_output(Output *out, SECURITY_ATTRIBUTES *attributes)
{
    return CreatePipe(&out->reader, &out->writer, attributes, PIPE_BYTES)
        && SetHandleInformation(out->reader, HANDLE_FLAG_INHERIT, 0);
}

static void observe_output(Output *out)
{
    if (!out->reader) return;
    out->peek_called = TRUE;
    out->peek_ok = PeekNamedPipe(out->reader, NULL, 0, NULL, &out->available, NULL);
    if (!out->peek_ok) {
        out->peek_error = GetLastError();
    } else if (out->available != 0) {
        const DWORD limit = out->available < PIPE_BYTES ? out->available : PIPE_BYTES;
        out->read_called = TRUE;
        out->read_ok = ReadFile(out->reader, out->data, limit, &out->bytes, NULL);
        if (!out->read_ok) out->read_error = GetLastError();
    }
}

static BOOL empty_output(const Output *out)
{
    return out->peek_called && !out->peek_ok && out->peek_error == ERROR_BROKEN_PIPE;
}

static BOOL launch_original(const Run *run, const Image *image, HANDLE job, HANDLE input,
                            HANDLE mapping, Slot *row, PROCESS_INFORMATION *owner)
{
    STARTUPINFOEXW info = {0};
    WCHAR argv[_countof(command_line)];
    WCHAR environment[_countof(row->environment)];
    HANDLE inherited[] = {input, row->out.writer, row->err.writer, mapping};
    SIZE_T bytes = 0;
    BOOL ok = FALSE;

    row->stage = "attribute-size";
    if (InitializeProcThreadAttributeList(NULL, 2, 0, &bytes) ||
        (row->error = GetLastError()) != ERROR_INSUFFICIENT_BUFFER || bytes == 0) return FALSE;
    row->error = 0;
    row->stage = "attribute-memory";
    info.lpAttributeList = HeapAlloc(GetProcessHeap(), 0, bytes);
    if (!info.lpAttributeList) return FALSE;
    row->stage = "attribute-init";
    BOOL initialized = InitializeProcThreadAttributeList(info.lpAttributeList, 2, 0, &bytes);
    if (!initialized) {
        row->error = GetLastError();
        goto done;
    }
    row->stage = "attribute-handle-list";
    if (!UpdateProcThreadAttribute(info.lpAttributeList, 0, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                                   inherited, sizeof(inherited), NULL, NULL)) {
        row->error = GetLastError();
        goto done;
    }
    row->stage = "attribute-job-list";
    if (!UpdateProcThreadAttribute(info.lpAttributeList, 0, PROC_THREAD_ATTRIBUTE_JOB_LIST,
                                   &job, sizeof(job), NULL, NULL)) {
        row->error = GetLastError();
        goto done;
    }
    info.StartupInfo.cb = (DWORD)sizeof(info);
    info.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    info.StartupInfo.hStdInput = input;
    info.StartupInfo.hStdOutput = row->out.writer;
    info.StartupInfo.hStdError = row->err.writer;
    memcpy(argv, command_line, sizeof(argv));
    memcpy(environment, row->environment, row->environment_bytes);
    row->stage = "pre-create-admission";
    int64_t admitted;
    if (!stamp(&admitted, &row->error)
        || admitted > run->aggregate_end - run->slot_ticks) goto done;
    row->stage = "create-suspended";
    ok = CreateProcessW(image->path, argv, NULL, NULL, TRUE,
                        DETACHED_PROCESS | CREATE_SUSPENDED | EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
                        environment, run->cwd, &info.StartupInfo, owner);
    if (!ok) row->error = GetLastError();
done:
    if (initialized) DeleteProcThreadAttributeList(info.lpAttributeList);
    HeapFree(GetProcessHeap(), 0, info.lpAttributeList);
    return ok;
}

static BOOL wait_budget(int64_t deadline, int64_t now, int64_t frequency, DWORD *milliseconds)
{
    if (now < 0 || now >= deadline) return FALSE;
    const uint64_t ticks = (uint64_t)(deadline - now);
    if (ticks > UINT64_MAX / 1000) return FALSE;
    const uint64_t ms = (1000 * ticks) / (uint64_t)frequency;
    *milliseconds = ms >= MAXDWORD ? MAXDWORD - 1 : (DWORD)ms;
    return TRUE;
}

static void request_cleanup(const Run *run, HANDLE job, HANDLE process, Slot *row)
{
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION counts;
    int64_t now;
    row->assisted = TRUE;
    row->kill_ok = TerminateJobObject(job, PTG_CRT_FAILURE_EXIT);
    if (!row->kill_ok) row->kill_error = GetLastError();
    if (!row->member_checked || !row->member) {
        /* Membership uncertainty is an error. This original is still suspended. */
        row->direct_kill = TRUE;
        row->direct_kill_ok = TerminateProcess(process, PTG_CRT_FAILURE_EXIT);
        if (!row->direct_kill_ok) row->direct_kill_error = GetLastError();
    }
    if (!stamp(&now, &row->cleanup_clock_error)
        || !wait_budget(run->aggregate_end, now, run->frequency, &row->cleanup_wait_ms)) return;
    row->cleanup_waited = TRUE;
    row->cleanup_wait = WaitForSingleObject(process, row->cleanup_wait_ms);
    if (row->cleanup_wait == WAIT_FAILED) row->cleanup_wait_error = GetLastError();
    if (!stamp(&now, &row->cleanup_clock_error) || now >= run->aggregate_end) return;
    row->cleanup_job_read = QueryInformationJobObject(job, JobObjectBasicAccountingInformation,
                                                      &counts, (DWORD)sizeof(counts), NULL);
    if (row->cleanup_job_read) row->cleanup_active = counts.ActiveProcesses;
    else row->cleanup_job_error = GetLastError();
}

static BOOL record_frame(Run *run, PtgCrtFrame *frame, Slot *row)
{
    /* Read only after this exact child's native terminal observation. */
    row->phase = InterlockedCompareExchange(&frame->phase, PTG_CRT_READY, PTG_CRT_READY);
    memcpy(row->raw_frame.request, frame->request, PTG_CRT_REQUEST_BYTES);
    memcpy(row->raw_frame.reply, frame->reply, PTG_CRT_REPLY_BYTES);
    row->frame_read = TRUE;
    row->frame_hashed = hash_bytes(run, row->raw_frame.request, PTG_CRT_REQUEST_BYTES, row->request_sha)
        && hash_bytes(run, row->raw_frame.reply, PTG_CRT_REPLY_BYTES, row->reply_sha);
    return row->frame_hashed;
}

static BOOL valid_reply(const Run *run, const Slot *row)
{
    const uint8_t *reply = row->raw_frame.reply;
    const uint64_t stamp_bits = ptg_crt_read_u64(reply + PTG_CRT_QPC_AT);
    const uint64_t earliest = row->before_resume > 0 ? (uint64_t)(row->before_resume - 1) : 0;
    return row->phase == PTG_CRT_READY
        && memcmp(row->request, row->raw_frame.request, PTG_CRT_REQUEST_BYTES) == 0
        && memcmp(reply, PTG_CRT_REPLY_MAGIC, PTG_CRT_MAGIC_BYTES) == 0
        && memcmp(reply + PTG_CRT_MAGIC_BYTES, row->request + PTG_CRT_MAGIC_BYTES,
                  PTG_CRT_REQUEST_BYTES - PTG_CRT_MAGIC_BYTES) == 0
        && ptg_crt_read_u64(reply + PTG_CRT_QPF_AT) == (uint64_t)run->frequency
        && ptg_crt_read_u32(reply + PTG_CRT_CRC_AT) == ptg_crt_reply_crc(reply)
        && stamp_bits <= (uint64_t)INT64_MAX && stamp_bits <= (uint64_t)row->after_wait + 1
        && stamp_bits >= earliest;
}

static BOOL process_identity(HANDLE process, DWORD *pid, uint64_t *epoch, DWORD *error)
{
    FILETIME creation, exit_time, kernel, user;
    *pid = GetProcessId(process);
    if (*pid == 0 || !GetProcessTimes(process, &creation, &exit_time, &kernel, &user)) {
        *error = GetLastError();
        return FALSE;
    }
    *epoch = filetime(creation);
    return *epoch != 0;
}

static void prepare_request(PtgCrtFrame *frame, Slot *row, const uint8_t *nonce)
{
    uint8_t *request = row->request;
    memcpy(request + PTG_CRT_MAGIC_AT, PTG_CRT_REQUEST_MAGIC, PTG_CRT_MAGIC_BYTES);
    ptg_crt_write_u32(request + PTG_CRT_VERSION_AT, PTG_CRT_VERSION);
    ptg_crt_write_u32(request + PTG_CRT_PID_AT, row->pid);
    ptg_crt_write_u64(request + PTG_CRT_EPOCH_AT, row->epoch);
    memcpy(request + PTG_CRT_NONCE_AT, nonce, PTG_CRT_NONCE_BYTES);
    memcpy(frame->request, request, PTG_CRT_REQUEST_BYTES);
    InterlockedExchange(&frame->phase, PTG_CRT_EMPTY);
}

static BOOL run_slot(Run *run, const Image *image, Slot *row)
{
    SECURITY_ATTRIBUTES inherit = {(DWORD)sizeof(inherit), NULL, TRUE};
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION limits = {0};
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION counts;
    PROCESS_INFORMATION owner = {0};
    HANDLE job = NULL, mapping = NULL, input = NULL;
    PtgCrtFrame *frame = NULL;
    uint8_t nonce[PTG_CRT_NONCE_BYTES];
    BOOL success = FALSE;
    BOOL same_terminal = FALSE;
    int64_t now;

    row->stage = "aggregate-admission";
    if (!stamp(&now, &row->error) || now > run->aggregate_end - run->slot_ticks) goto done;
    row->stage = "new-owned-job";
    job = CreateJobObjectW(NULL, NULL);
    if (!job) goto native_failure;
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if (!SetInformationJobObject(job, JobObjectExtendedLimitInformation, &limits, (DWORD)sizeof(limits))) goto native_failure;

    row->stage = "private-frame";
    mapping = CreateFileMappingW(INVALID_HANDLE_VALUE, &inherit, PAGE_READWRITE, 0, PTG_CRT_PAGE_BYTES, NULL);
    if (!mapping) goto native_failure;
    frame = MapViewOfFile(mapping, FILE_MAP_WRITE, 0, 0, PTG_CRT_PAGE_BYTES);
    if (!frame) goto native_failure;
    row->stage = "nonce";
    row->rng_status = BCryptGenRandom(NULL, nonce, PTG_CRT_NONCE_BYTES, BCRYPT_USE_SYSTEM_PREFERRED_RNG);
    if (row->rng_status != 0) goto done;
    row->stage = "nonce-reuse";
    for (unsigned i = 0; i < run->nonce_count; ++i) {
        if (memcmp(run->nonces[i], nonce, sizeof(nonce)) == 0) goto done;
    }
    memcpy(run->nonces[run->nonce_count++], nonce, sizeof(nonce));
    make_environment(run, row, mapping, nonce);
    row->stage = "environment-digest";
    row->environment_hashed = hash_bytes(run, row->environment, row->environment_bytes, row->environment_sha);
    if (!row->environment_hashed) goto done;

    row->stage = "exact-stdio";
    input = CreateFileW(L"NUL", GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE,
                        &inherit, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
    if (input == INVALID_HANDLE_VALUE || !prepare_output(&row->out, &inherit)
        || !prepare_output(&row->err, &inherit)) goto native_failure;
    row->stage = "before-create-cutoff";
    if (!stamp(&now, &row->error) || now > run->aggregate_end - run->slot_ticks) goto done;
    if (!launch_original(run, image, job, input, mapping, row, &owner)) goto done;
    row->created = TRUE;
    row->pid = owner.dwProcessId;
    close_slot_handle(&input, row);
    close_slot_handle(&row->out.writer, row);
    close_slot_handle(&row->err.writer, row);
    row->stage = "parent-handle-release";
    if (row->release_failed) goto done;

    row->stage = "original-job-membership";
    row->member_checked = IsProcessInJob(owner.hProcess, job, &row->member);
    if (!row->member_checked) goto native_failure;
    if (!row->member) goto done;
    row->stage = "original-pid-epoch";
    row->has_epoch = process_identity(owner.hProcess, &row->original_pid, &row->epoch, &row->error);
    if (!row->has_epoch || row->pid != row->original_pid) goto done;
    row->stage = "original-image";
    row->image_joined = join_original_image(owner.hProcess, image);
    if (!row->image_joined) goto done;
    row->stage = "original-suspended-poll";
    row->pre_resume_status = WaitForSingleObject(owner.hProcess, 0);
    row->pre_resume_error = row->pre_resume_status == WAIT_FAILED ? GetLastError() : 0;
    row->pre_resume_polled = TRUE;
    if (row->pre_resume_status != WAIT_TIMEOUT) goto done;
    prepare_request(frame, row, nonce);

    row->stage = "native-resume";
    if (!stamp(&row->before_resume, &row->error)
        || row->before_resume > run->aggregate_end - run->slot_ticks) goto done;
    row->resume_count = ResumeThread(owner.hThread);
    row->resume_error = row->resume_count == MAXDWORD ? GetLastError() : 0;
    row->resumed = TRUE;
    row->cutoff = row->before_resume + run->slot_ticks;
    if (!stamp(&row->after_resume, &row->error) || row->resume_count != 1
        || row->after_resume < row->before_resume || row->after_resume >= row->cutoff) goto done;

    row->stage = "native-original-wait";
    if (!stamp(&row->before_wait, &row->error) || row->before_wait < row->after_resume
        || !wait_budget(row->cutoff, row->before_wait, run->frequency, &row->wait_ms)) goto done;
    row->status = WaitForSingleObject(owner.hProcess, row->wait_ms);
    row->wait_error = row->status == WAIT_FAILED ? GetLastError() : 0;
    row->waited = TRUE;
    if (!stamp(&row->after_wait, &row->error) || row->status != WAIT_OBJECT_0
        || row->after_wait < row->before_wait || row->after_wait >= row->cutoff) goto done;

    row->stage = "native-terminal-evidence";
    row->exit_read = GetExitCodeProcess(owner.hProcess, &row->exit_code);
    if (!row->exit_read) goto native_failure;
    row->terminal_identity = process_identity(owner.hProcess, &row->terminal_pid, &row->terminal_epoch, &row->error);
    same_terminal = row->terminal_identity && row->terminal_pid == row->pid && row->terminal_epoch == row->epoch;
    if (!same_terminal) goto done;
    row->job_read = QueryInformationJobObject(job, JobObjectBasicAccountingInformation,
                                              &counts, (DWORD)sizeof(counts), NULL);
    if (!row->job_read) goto native_failure;
    row->active = counts.ActiveProcesses;
    row->total = counts.TotalProcesses;
    if (!stamp(&row->after_job, &row->error) || row->after_job < row->after_wait
        || row->after_job >= row->cutoff || row->total != 1 || row->active != 0 || row->exit_code != 0) goto done;
    row->stage = "frame";
    if (!record_frame(run, frame, row) || !valid_reply(run, row)) goto done;
    row->stage = "stdio";
    observe_output(&row->out);
    observe_output(&row->err);
    if (!empty_output(&row->out) || !empty_output(&row->err)) goto done;
    success = TRUE;
    goto done;

native_failure:
    row->error = GetLastError();
done:
    if (row->created && (!row->waited || row->status != WAIT_OBJECT_0 || !row->job_read || row->active != 0)) {
        request_cleanup(run, job, owner.hProcess, row);
    }
    if (frame && !row->frame_read &&
        ((row->waited && row->status == WAIT_OBJECT_0) || (row->cleanup_waited && row->cleanup_wait == WAIT_OBJECT_0))) {
        (void)record_frame(run, frame, row);
    }
    close_slot_handle(&row->out.writer, row);
    close_slot_handle(&row->err.writer, row);
    if (!row->out.peek_called) observe_output(&row->out);
    if (!row->err.peek_called) observe_output(&row->err);
    close_slot_handle(&row->out.reader, row);
    close_slot_handle(&row->err.reader, row);
    if (frame && !UnmapViewOfFile(frame) && !row->release_failed) {
        row->release_error = GetLastError();
        row->release_failed = TRUE;
    }
    close_slot_handle(&mapping, row);
    close_slot_handle(&input, row);
    close_slot_handle(&owner.hThread, row);
    close_slot_handle(&owner.hProcess, row);
    close_slot_handle(&job, row);
    if (success && row->release_failed) {
        row->stage = "release-owned-resources";
        return FALSE;
    }
    if (success) {
        row->stage = "aggregate-settlement";
        if (!stamp(&row->settled, &row->error) || row->settled < row->after_job ||
            row->settled >= run->aggregate_end) return FALSE;
        row->stage = "accepted";
    }
    return success;
}

static void blob(FILE *out, const char *name, const void *data, size_t size)
{
    const uint8_t *bytes = data;
    static const char hex[] = "0123456789abcdef";
    fprintf(out, ",\"%s\":\"", name);
    for (size_t i = 0; i < size; ++i) {
        fputc(hex[bytes[i] >> 4], out);
        fputc(hex[bytes[i] & 15], out);
    }
    fputc('"', out);
}

static void wide_blob(FILE *out, const char *name, const WCHAR *value)
{
    blob(out, name, value, (wcslen(value) + 1) * sizeof(WCHAR));
}

static void output_row(FILE *out, const char *name, const Output *value)
{
    fprintf(out, ",\"%s\":{\"peek_called\":%d,\"peek_ok\":%d,\"peek_error\":%lu,\"available\":%lu,"
                 "\"read_called\":%d,\"read_ok\":%d,\"read_error\":%lu,\"bytes\":%lu,\"truncated\":%d",
            name, !!value->peek_called, !!value->peek_ok, value->peek_error, value->available,
            !!value->read_called, !!value->read_ok, value->read_error, value->bytes, value->available > PIPE_BYTES);
    blob(out, "data_hex", value->data, value->bytes);
    fputc('}', out);
}

static void image_row(FILE *out, const char *arm, const Image *image)
{
    fprintf(out, ",\"%s\":{\"volume\":%" PRIu64, arm, (uint64_t)image->identity.VolumeSerialNumber);
    wide_blob(out, "application_utf16le_hex", image->path);
    blob(out, "application_sha256", image->path_sha, SHA_BYTES);
    blob(out, "image_sha256", image->sha, SHA_BYTES);
    blob(out, "file_id", image->identity.FileId.Identifier, sizeof(image->identity.FileId.Identifier));
    fputc('}', out);
}

static BOOL durable(FILE *out)
{
    return !ferror(out) && fflush(out) == 0 && FlushFileBuffers((HANDLE)_get_osfhandle(_fileno(out)));
}

static BOOL write_slot(FILE *out, unsigned slot, char role, const Slot *row, BOOL accepted)
{
    fprintf(out, "{\"kind\":\"slot\",\"slot\":%u,\"pair\":%u,\"arm\":\"%c\",\"attempted\":%d,\"accepted\":%d",
            slot, (slot + 1) / 2, role, row != NULL, !!accepted);
    if (row != NULL) {
        fprintf(out, ",\"stage\":\"%s\",\"error\":%lu,\"rng_status\":%ld,\"created\":%d"
                     ",\"member_checked\":%d,\"member\":%d,\"has_epoch\":%d,\"image_joined\":%d"
                     ",\"pid\":%lu,\"original_pid\":%lu,\"epoch\":%" PRIu64 ",\"terminal_identity\":%d"
                     ",\"terminal_pid\":%lu,\"terminal_epoch\":%" PRIu64 ",\"credential_handle\":%" PRIu64,
                row->stage, row->error, (long)row->rng_status, !!row->created,
                !!row->member_checked, !!row->member, !!row->has_epoch, !!row->image_joined,
                row->pid, row->original_pid, row->epoch, !!row->terminal_identity, row->terminal_pid,
                row->terminal_epoch, (uint64_t)row->credential_handle);
        blob(out, "environment_utf16le_hex", row->environment, row->environment_bytes);
        blob(out, "environment_sha256", row->environment_sha, SHA_BYTES);
        fprintf(out, ",\"environment_bytes\":%lu,\"environment_hashed\":%d", row->environment_bytes,
                !!row->environment_hashed);
        blob(out, "expected_request_hex", row->request, PTG_CRT_REQUEST_BYTES);
        fprintf(out, ",\"before_resume\":%" PRId64 ",\"after_resume\":%" PRId64 ",\"cutoff\":%" PRId64
                     ",\"before_wait\":%" PRId64 ",\"after_wait\":%" PRId64 ",\"after_job\":%" PRId64 ",\"settled\":%" PRId64
                     ",\"pre_resume_polled\":%d,\"pre_resume_status\":%lu,\"pre_resume_error\":%lu"
                     ",\"resume_called\":%d,\"resume_count\":%lu,\"resume_error\":%lu"
                     ",\"waited\":%d,\"wait_ms\":%lu,\"wait_status\":%lu,\"wait_error\":%lu"
                     ",\"exit_read\":%d,\"exit_code\":%lu,\"job_read\":%d,\"total\":%lu,\"active\":%lu",
                row->before_resume, row->after_resume, row->cutoff, row->before_wait, row->after_wait, row->after_job, row->settled,
                !!row->pre_resume_polled, row->pre_resume_status, row->pre_resume_error,
                !!row->resumed, row->resume_count, row->resume_error, !!row->waited, row->wait_ms,
                row->status, row->wait_error, !!row->exit_read, row->exit_code, !!row->job_read, row->total, row->active);
        fprintf(out, ",\"frame_read\":%d,\"frame_hashed\":%d,\"phase\":%ld", !!row->frame_read, !!row->frame_hashed, row->phase);
        if (row->frame_read) {
            fprintf(out, ",\"request_bytes\":%u,\"reply_bytes\":%u", (unsigned)PTG_CRT_REQUEST_BYTES,
                    (unsigned)PTG_CRT_REPLY_BYTES);
            blob(out, "request_hex", row->raw_frame.request, PTG_CRT_REQUEST_BYTES);
            blob(out, "request_sha256", row->request_sha, SHA_BYTES);
            blob(out, "reply_hex", row->raw_frame.reply, PTG_CRT_REPLY_BYTES);
            blob(out, "reply_sha256", row->reply_sha, SHA_BYTES);
        }
        output_row(out, "stdout", &row->out);
        output_row(out, "stderr", &row->err);
        fprintf(out, ",\"assisted\":%d,\"kill_ok\":%d,\"kill_error\":%lu,\"direct_kill\":%d"
                     ",\"direct_kill_ok\":%d,\"direct_kill_error\":%lu,\"cleanup_waited\":%d"
                     ",\"cleanup_wait_ms\":%lu,\"cleanup_wait\":%lu,\"cleanup_wait_error\":%lu"
                     ",\"cleanup_clock_error\":%lu,\"cleanup_job_read\":%d,\"cleanup_job_error\":%lu"
                     ",\"cleanup_active\":%lu,\"release_failed\":%d,\"release_error\":%lu",
                !!row->assisted, !!row->kill_ok, row->kill_error, !!row->direct_kill, !!row->direct_kill_ok,
                row->direct_kill_error, !!row->cleanup_waited, row->cleanup_wait_ms, row->cleanup_wait,
                row->cleanup_wait_error, row->cleanup_clock_error, !!row->cleanup_job_read, row->cleanup_job_error,
                row->cleanup_active, !!row->release_failed, row->release_error);
    }
    fputs("}\n", out);
    return durable(out);
}

static FILE *open_journal(const WCHAR *directory)
{
    static const WCHAR suffix[] = L"\\raw.jsonl";
    WCHAR name[MAX_PATH];
    const size_t length = wcslen(directory);
    if (!absolute_path(directory) || length + _countof(suffix) > _countof(name)
        || !CreateDirectoryW(directory, NULL)) return NULL;
    memcpy(name, directory, length * sizeof(WCHAR));
    memcpy(name + length, suffix, sizeof(suffix));
    HANDLE file = CreateFileW(name, GENERIC_WRITE, FILE_SHARE_READ, NULL, CREATE_NEW,
                              FILE_FLAG_OPEN_REPARSE_POINT, NULL);
    if (file == INVALID_HANDLE_VALUE) return NULL;
    const int fd = _open_osfhandle((intptr_t)file, _O_BINARY | _O_WRONLY);
    if (fd < 0) {
        close_handle(&file);
        return NULL;
    }
    FILE *out = _fdopen(fd, "wb");
    if (out == NULL) _close(fd);
    return out;
}

static const char *prepare_run(Run *run, Image images[2], WCHAR **argv, DWORD *error)
{
    LARGE_INTEGER frequency;
    BOOL in_job = FALSE;
    if (!IsProcessInJob(GetCurrentProcess(), NULL, &in_job)) {
        *error = GetLastError();
        return "outer-job-query";
    }
    if (!in_job) return "outer-job-missing";
    if (!QueryPerformanceFrequency(&frequency)) {
        *error = GetLastError();
        return "clock-frequency";
    }
    if (frequency.QuadPart <= 0 || frequency.QuadPart > INT64_MAX / 180) return "clock-range";
    if (!stamp(&run->start, error) || run->start > INT64_MAX - frequency.QuadPart * 180) return "clock-start";
    run->frequency = frequency.QuadPart;
    run->slot_ticks = run->frequency + run->frequency / 2;
    run->aggregate_end = run->start + run->frequency * 180;
    SetLastError(ERROR_SUCCESS);
    if (GetEnvironmentVariableW(PTG_CRT_PROBE_ENV, NULL, 0) != 0 || GetLastError() != ERROR_ENVVAR_NOT_FOUND) {
        return "inherited-probe-optin";
    }
    const UINT windows_chars = GetWindowsDirectoryW(run->windows, MAX_PATH);
    if (windows_chars == 0 || windows_chars >= MAX_PATH || !absolute_path(run->windows)) return "system-root";
    if (BCryptOpenAlgorithmProvider(&run->sha, BCRYPT_SHA256_ALGORITHM, NULL, 0) != 0) return "sha-provider";
    if (!pin_image(run, &images[0], argv[1], argv[2])) return "arm-A-pin";
    if (!pin_image(run, &images[1], argv[3], argv[4])) return "arm-B-pin";
    if (same_image(&images[0].identity, &images[1].identity)
        || memcmp(images[0].sha, images[1].sha, SHA_BYTES) == 0) return "arms-not-distinct";
    return NULL;
}

int wmain(int argc, WCHAR **argv)
{
    Run run = {0};
    Image images[2] = {0};
    DWORD preflight_error = 0;
    const char *preflight;
    unsigned attempts = 0, accepted = 0;

    if (argc != 7 || !absolute_path(argv[5]) || wcslen(argv[5]) >= MAX_PATH) return 2;
    run.cwd = argv[5];
    run.journal = open_journal(argv[6]);
    if (!run.journal) return 2;
    preflight = prepare_run(&run, images, argv, &preflight_error);
    uint8_t argv_sha[SHA_BYTES], cwd_sha[SHA_BYTES];
    if (preflight == NULL &&
        (!hash_bytes(&run, command_line, (ULONG)sizeof(command_line), argv_sha)
        || !hash_bytes(&run, run.cwd, (ULONG)((wcslen(run.cwd) + 1) * sizeof(WCHAR)), cwd_sha))) {
        preflight = "command-context-digest";
    }
    fprintf(run.journal, "{\"kind\":\"run\",\"schema\":\"ptg-crt-controller-003-v1\",\"schedule_sha256\":\"%s\""
                         ",\"frequency\":%" PRId64 ",\"start\":%" PRId64 ",\"aggregate_end\":%" PRId64
                         ",\"slot_ticks\":%" PRId64 ",\"planned\":%u,\"preflight\":\"%s\",\"error\":%lu",
            schedule_sha, run.frequency, run.start, run.aggregate_end, run.slot_ticks, (unsigned)SLOTS,
            preflight ? preflight : "accepted", preflight_error);
    if (preflight == NULL) {
        wide_blob(run.journal, "argv_utf16le_hex", command_line);
        blob(run.journal, "argv_sha256", argv_sha, SHA_BYTES);
        wide_blob(run.journal, "cwd_utf16le_hex", run.cwd);
        blob(run.journal, "cwd_sha256", cwd_sha, SHA_BYTES);
        image_row(run.journal, "A", &images[0]);
        image_row(run.journal, "B", &images[1]);
    }
    fputs("}\n", run.journal);
    BOOL healthy = durable(run.journal);
    BOOL dispatch = preflight == NULL && healthy;
    for (unsigned index = 0; index < (unsigned)SLOTS && healthy; ++index) {
        const char role = pairs[index / 2][index % 2];
        if (dispatch) {
            Slot row = {0};
            ++attempts;
            dispatch = run_slot(&run, &images[role == 'A' ? 0 : 1], &row);
            if (dispatch) ++accepted;
            healthy = write_slot(run.journal, index + 1, role, &row, dispatch);
        } else {
            healthy = write_slot(run.journal, index + 1, role, NULL, FALSE);
        }
    }
    int64_t end = 0;
    DWORD final_clock_error = 0;
    BOOL complete = healthy && accepted == (unsigned)SLOTS && stamp(&end, &final_clock_error) && end < run.aggregate_end;
    if (healthy) {
        fprintf(run.journal, "{\"kind\":\"terminal\",\"attempts\":%u,\"accepted\":%u,\"complete\":%d"
                             ",\"end\":%" PRId64 ",\"clock_error\":%lu}\n",
                attempts, accepted, !!complete, end, final_clock_error);
        complete = durable(run.journal) && complete;
    }
    if (fclose(run.journal) != 0) complete = FALSE;
    if (!close_handle(&images[0].file)) complete = FALSE;
    if (!close_handle(&images[1].file)) complete = FALSE;
    if (run.sha != NULL && BCryptCloseAlgorithmProvider(run.sha, 0) != 0) complete = FALSE;
    return complete ? 0 : 2;
}
