// SPDX-License-Identifier: Apache-2.0
#ifndef PTG220_NATIVE_SYSCALL_H
#define PTG220_NATIVE_SYSCALL_H

#if !defined(__linux__) || !defined(__x86_64__) || defined(__ILP32__)
#error "The private ordinary Linux candidate is x86-64 Linux LP64 only"
#endif

/* Keep the post-clone paths independent of libc and TLS errno. */
static inline long native(long number, long a, long b, long c, long d, long e) {
    register long r10 __asm__("r10") = d;
    register long r8 __asm__("r8") = e;
    long result;
    __asm__ volatile("syscall" : "=a" (result)
                     : "a" (number), "D" (a), "S" (b), "d" (c), "r" (r10), "r" (r8)
                     : "rcx", "r11", "memory", "cc");
    return result;
}

#endif
