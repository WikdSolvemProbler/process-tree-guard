// SPDX-License-Identifier: Apache-2.0
#ifndef PTG220_DESCRIPTORS_H
#define PTG220_DESCRIPTORS_H

#include <stddef.h>

struct ptg220_descriptor {
    int target;
    int held;
};

struct ptg220_descriptors {
    struct ptg220_descriptor *entries;
    size_t count;
    size_t capacity;
};

int ptg220_descriptor_capture(struct ptg220_descriptors *set);
int ptg220_descriptor_take(struct ptg220_descriptors *set, int target, int held);
int ptg220_descriptor_restore(struct ptg220_descriptors *set);
void ptg220_descriptor_drop(struct ptg220_descriptors *set);

#endif
