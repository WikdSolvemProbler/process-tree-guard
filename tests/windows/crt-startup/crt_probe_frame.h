#ifndef PTG_CRT_PROBE_FRAME_H
#define PTG_CRT_PROBE_FRAME_H

#include <stddef.h>
#include <stdint.h>
#include <windows.h>

#if !defined(_M_X64)
#error The private CRT probe requires the pinned AMD64 toolchain.
#endif

#define PTG_CRT_PROBE_ENV L"PROCESS_TREE_GUARD_CRT_PROBE_201"
#define PTG_CRT_REQUEST_MAGIC "PTGCPRQ1"
#define PTG_CRT_REPLY_MAGIC "PTGCPRP1"
#define PTG_CRT_FAILURE_EXIT UINT32_C(0xe0000201)

enum {
    PTG_CRT_VERSION = 1,
    PTG_CRT_PAGE_BYTES = 4096,
    PTG_CRT_MAGIC_BYTES = 8,
    PTG_CRT_NONCE_BYTES = 16,
    PTG_CRT_HANDLE_HEX = 16,
    PTG_CRT_CREDENTIAL_CHARS = PTG_CRT_HANDLE_HEX + 1 + 2 * PTG_CRT_NONCE_BYTES,

    PTG_CRT_EMPTY = 0,
    PTG_CRT_WRITING = 1,
    PTG_CRT_READY = 2,
    PTG_CRT_FAILED = 3,

    /* Offsets within the raw request and reply, not within the mapping. */
    PTG_CRT_MAGIC_AT = 0,
    PTG_CRT_VERSION_AT = 8,
    PTG_CRT_PID_AT = 12,
    PTG_CRT_EPOCH_AT = 16,
    PTG_CRT_NONCE_AT = 24,
    PTG_CRT_REQUEST_BYTES = 40,
    PTG_CRT_QPC_AT = 40,
    PTG_CRT_QPF_AT = 48,
    PTG_CRT_CRC_AT = 56,
    PTG_CRT_REPLY_BYTES = 60
};

typedef struct {
    volatile LONG phase;
    uint8_t request[PTG_CRT_REQUEST_BYTES];
    uint8_t reply[PTG_CRT_REPLY_BYTES];
} PtgCrtFrame;

_Static_assert(sizeof(LONG) == 4, "phase is one native 32-bit word");
_Static_assert(_Alignof(PtgCrtFrame) >= 4, "phase requires natural alignment");
_Static_assert(offsetof(PtgCrtFrame, phase) == 0, "phase offset");
_Static_assert(offsetof(PtgCrtFrame, request) == 4, "request offset");
_Static_assert(offsetof(PtgCrtFrame, reply) == 44, "reply offset");
_Static_assert(sizeof(PtgCrtFrame) == 104, "frame size");

static inline uint32_t ptg_crt_read_u32(const uint8_t *bytes)
{
    return (uint32_t)bytes[0] | ((uint32_t)bytes[1] << 8)
         | ((uint32_t)bytes[2] << 16) | ((uint32_t)bytes[3] << 24);
}

static inline uint64_t ptg_crt_read_u64(const uint8_t *bytes)
{
    return ptg_crt_read_u32(bytes) | ((uint64_t)ptg_crt_read_u32(bytes + 4) << 32);
}

static inline void ptg_crt_write_u32(uint8_t *bytes, uint32_t value)
{
    for (unsigned i = 0; i < 4; ++i) {
        bytes[i] = (uint8_t)(value >> (8 * i));
    }
}

static inline void ptg_crt_write_u64(uint8_t *bytes, uint64_t value)
{
    ptg_crt_write_u32(bytes, (uint32_t)value);
    ptg_crt_write_u32(bytes + 4, (uint32_t)(value >> 32));
}

static inline uint32_t ptg_crt_reply_crc(const uint8_t *reply)
{
    uint32_t crc = UINT32_MAX;
    for (size_t i = 0; i < PTG_CRT_CRC_AT; ++i) {
        crc ^= reply[i];
        for (unsigned bit = 0; bit < 8; ++bit) {
            const uint32_t mask = UINT32_C(0) - (crc & UINT32_C(1));
            crc = (crc >> 1) ^ (UINT32_C(0xedb88320) & mask);
        }
    }
    return ~crc;
}

#endif
