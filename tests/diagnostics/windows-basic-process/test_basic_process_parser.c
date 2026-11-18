/* Builds only the exact staged decoder, with no native query entry point. */
#define BASIC_PROBE_PARSER_ONLY 1
#include "basic_process_probe.c"

#define FIRST 0u
#define SECOND 48u
#define NAME 96u
#define USED 104u

__declspec(align(16)) static uint8_t fixture[USED];
static BasicRow result[2];

static SYSTEM_BASICPROCESS_INFORMATION read_head(size_t offset)
{
    SYSTEM_BASICPROCESS_INFORMATION value;
    memcpy(&value, fixture + offset, HEAD_BYTES);
    return value;
}

static void put_head(size_t offset, const SYSTEM_BASICPROCESS_INFORMATION *value)
{
    memcpy(fixture + offset, value, HEAD_BYTES);
}

static void seed(void)
{
    memset(fixture, 0, sizeof(fixture));
    SYSTEM_BASICPROCESS_INFORMATION zero = {0};
    zero.NextEntryOffset = SECOND;
    put_head(FIRST, &zero);

    SYSTEM_BASICPROCESS_INFORMATION other = {0};
    other.UniqueProcessId = (HANDLE)(uintptr_t)42;
    other.InheritedFromUniqueProcessId = (HANDLE)(uintptr_t)4;
    other.SequenceNumber = UINT64_C(9007199254740993);
    other.ImageName.Length = 4;
    other.ImageName.MaximumLength = 6;
    other.ImageName.Buffer = (PWSTR)(fixture + NAME);
    put_head(SECOND, &other);
    /* Lossless surrogate prefix without relying on a Unicode conversion. */
    fixture[NAME + 1] = 0xd8;
    fixture[NAME + 2] = 'x';
}

static int expect(const char *name, const char *reason, size_t index, size_t returned,
                  size_t row_cap)
{
    size_t count = SIZE_MAX;
    const ParseResult value = decode_rows(fixture, returned, sizeof(fixture), result,
                                          row_cap, &count);
    if (!value.reason || strcmp(value.reason, reason) || value.index != index || count != index) {
        fprintf(stderr, "%s: got %s at %zu with %zu rows\n", name,
                value.reason ? value.reason : "success", value.index, count);
        return 1;
    }
    return 0;
}

int main(void)
{
    size_t count = 0;
    seed();
    const ParseResult ok = decode_rows(fixture, USED, sizeof(fixture), result, 2, &count);
    if (ok.reason || count != 2 || result[0].pid != 0 || result[0].length != 0
            || result[0].buffer_present || result[0].next_offset != SECOND
            || result[1].pid != 42 || result[1].parent != 4 || result[1].next_offset != 0
            || result[1].sequence != UINT64_C(9007199254740993)
            || result[1].name_offset != NAME || result[1].length != 4 || result[1].maximum != 6
            || !result[1].buffer_present)
        return 1;
    int problems = 0;
    problems += expect("short", "returned_length", 0, HEAD_BYTES - 1, 2);
    problems += expect("oversize", "returned_length", 0, USED + 1u, 2);
    problems += expect("capacity", "row_capacity_before_last_entry", 1, USED, 1);

    const ULONG bad_next[] = {8u, 49u, 64u};
    for (size_t i = 0; i < ARRAYSIZE(bad_next); ++i) {
        seed();
        SYSTEM_BASICPROCESS_INFORMATION row = read_head(FIRST);
        row.NextEntryOffset = bad_next[i];
        put_head(FIRST, &row);
        problems += expect("bad link", "next_entry_offset", 0, USED, 2);
    }
    seed();
    SYSTEM_BASICPROCESS_INFORMATION row = read_head(FIRST);
    row.NextEntryOffset = 0;
    put_head(FIRST, &row);
    problems += expect("early terminator", "unexplained_record_extent", 0, USED, 2);

    seed();
    row = read_head(SECOND);
    row.UniqueProcessId = (HANDLE)(uintptr_t)UINT64_C(0x100000000);
    put_head(SECOND, &row);
    problems += expect("wide PID", "pid_width", 1, USED, 2);
    row.UniqueProcessId = (HANDLE)(uintptr_t)42;
    row.InheritedFromUniqueProcessId = (HANDLE)(uintptr_t)UINT64_C(0x100000000);
    put_head(SECOND, &row);
    problems += expect("wide parent PID", "pid_width", 1, USED, 2);

    seed();
    row = read_head(SECOND);
    row.UniqueProcessId = NULL;
    put_head(SECOND, &row);
    problems += expect("duplicate PID", "duplicate_pid", 1, USED, 2);
    seed();
    row = read_head(SECOND);
    row.ImageName.Length = 0;
    put_head(SECOND, &row);
    problems += expect("non-idle empty", "unexpected_empty_name", 1, USED, 2);

    seed();
    row = read_head(SECOND);
    row.ImageName.Length = 3;
    put_head(SECOND, &row);
    problems += expect("odd UTF16 length", "utf16_count", 1, USED, 2);
    row.ImageName.Length = 8;
    put_head(SECOND, &row);
    problems += expect("length over max", "utf16_count", 1, USED, 2);

    seed();
    row = read_head(SECOND);
    row.ImageName.Buffer = NULL;
    put_head(SECOND, &row);
    problems += expect("null counted buffer", "null_utf16_buffer", 1, USED, 2);
    row.ImageName.Buffer = (PWSTR)(fixture + NAME + 1u);
    put_head(SECOND, &row);
    problems += expect("misaligned name", "utf16_pointer", 1, USED, 2);
    row.ImageName.Buffer = (PWSTR)((uintptr_t)fixture + USED + 2u);
    put_head(SECOND, &row);
    problems += expect("pointer outside buffer", "utf16_pointer", 1, USED, 2);
    row.ImageName.Buffer = (PWSTR)(fixture + SECOND);
    put_head(SECOND, &row);
    problems += expect("pointer in header", "utf16_record_bounds", 1, USED, 2);
    row.ImageName.Buffer = (PWSTR)(fixture + USED - 2u);
    put_head(SECOND, &row);
    problems += expect("maximum crosses end", "utf16_record_bounds", 1, USED, 2);

    seed();
    fixture[NAME + 1] = 0;
    problems += expect("embedded NUL", "embedded_utf16_nul", 1, USED, 2);
    /* A non-null zero-length name still needs a pointer inside the
       returned bytes and inside its own record. Both were accepted in 001. */
    seed();
    row = read_head(FIRST);
    row.NextEntryOffset = 0;
    row.ImageName.Buffer = (PWSTR)(fixture + HEAD_BYTES);
    put_head(FIRST, &row);
    problems += expect("PID0 pointer one past returned buffer", "utf16_pointer", 0, HEAD_BYTES, 2);

    seed();
    row = read_head(FIRST);
    row.ImageName.Buffer = (PWSTR)(fixture + SECOND);
    put_head(FIRST, &row);
    problems += expect("PID0 pointer one past record", "utf16_record_bounds", 0, USED, 2);

    if (problems)
        return 1;
    puts("Basic parser: one exact two-row positive and all 21 bounded malformed cases passed; no NT call.");
    return 0;
}
