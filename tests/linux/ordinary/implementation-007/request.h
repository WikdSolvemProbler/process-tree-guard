// SPDX-License-Identifier: Apache-2.0
#ifndef PTG220_REQUEST_H
#define PTG220_REQUEST_H

#include <stddef.h>
#include <stdint.h>
#include <sys/resource.h>

struct ptg220_request {
    void *mapping;
    size_t length;
    const char *path;
    char **argv;
    char **envp;
    struct rlimit stack;
    uint64_t signal_mask;
    unsigned int closed_stdio;
    const void *security_context;
    unsigned int security_bytes;
    int security_error;
};

int ptg220_request_capture(const char *path, char *const argv[], char *const envp[]);
int ptg220_request_read(int fd, struct ptg220_request *request);
int ptg220_verify_inherited_security(const struct ptg220_request *request);
void ptg220_request_close(struct ptg220_request *request);

#endif
