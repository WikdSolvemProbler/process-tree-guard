// SPDX-License-Identifier: Apache-2.0
#define _GNU_SOURCE
#include "evidence.h"

#include <errno.h>
#include <fcntl.h>
#include <linux/memfd.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <unistd.h>

static const int seals = F_SEAL_SEAL | F_SEAL_SHRINK | F_SEAL_GROW | F_SEAL_WRITE;

int ptg220_evidence_seal(const struct ptg220_evidence *evidence) {
    const int fd = (int)syscall(SYS_memfd_create, "ptg-ordinary-native-receipt",
                                MFD_CLOEXEC | MFD_ALLOW_SEALING);
    if (fd < 0) return -errno;
    const ssize_t bytes = write(fd, evidence, sizeof(*evidence));
    int result = bytes < 0 ? -errno : bytes == (ssize_t)sizeof(*evidence) ? 0 : -EIO;
    if (result == 0 && fcntl(fd, F_ADD_SEALS, seals) < 0) result = -errno;
    if (result == 0) return fd;
    (void)close(fd);
    return result;
}

int ptg220_evidence_read(int fd, struct ptg220_evidence *evidence) {
    const int present = fcntl(fd, F_GET_SEALS);
    if (present < 0) return -errno;
    if ((present & seals) != seals) return -EPROTO;
    struct stat status;
    if (fstat(fd, &status) < 0) return -errno;
    if (!S_ISREG(status.st_mode) || status.st_size != (off_t)sizeof(*evidence)) return -EPROTO;
    const ssize_t bytes = pread(fd, evidence, sizeof(*evidence), 0);
    if (bytes < 0) return -errno;
    if (bytes != (ssize_t)sizeof(*evidence) || evidence->version != PTG220_EVIDENCE_VERSION) return -EPROTO;
    return 0;
}
