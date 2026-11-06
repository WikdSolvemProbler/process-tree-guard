#define _DARWIN_C_SOURCE 1

#if !defined(__APPLE__) || !defined(__aarch64__)
#error This proof is restricted to the pinned arm64 Darwin hosted image.
#endif

#include <errno.h>
#include <fcntl.h>
#include <grp.h>
#include <inttypes.h>
#include <libproc.h>
#include <membership.h>
#include <poll.h>
#include <pwd.h>
#include <signal.h>
#include <stdbool.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/event.h>
#include <sys/mman.h>
#include <sys/proc.h>
#include <sys/proc_info.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/sysctl.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

enum { rounds = 20, controller_seconds = 100, backup_seconds = 125, step_ms = 2500 };

struct account {
    const char *name;
    uid_t uid;
    gid_t gid;
};

static const struct account worker = {"ptgproofworker", 62081, 62081};
static const struct account peer = {"ptgproofpeer", 62082, 62082};

static void require(bool good, const char *what)
{
    if (!good) {
        fprintf(stderr, "uid-proof: %s (errno=%d)\n", what, errno);
        _exit(1);
    }
}

static int64_t now_ms(void)
{
    struct timespec t;
    require(clock_gettime(CLOCK_MONOTONIC, &t) == 0, "monotonic clock unavailable");
    return t.tv_sec * INT64_C(1000) + t.tv_nsec / 1000000;
}

static void nap(void)
{
    const struct timespec delay = {0, 10000000};
    (void)nanosleep(&delay, NULL);
}

static bool same_env(const char *name, const char *expected)
{
    const char *value = getenv(name);
    return value != NULL && strcmp(value, expected) == 0;
}

static bool decimal(const char *value)
{
    return value != NULL && value[0] >= '1' && value[0] <= '9' &&
        strlen(value) <= 20 && strspn(value, "0123456789") == strlen(value);
}

static bool sys_string(const char *name, char *value, size_t size)
{
    return sysctlbyname(name, value, &size, NULL, 0) == 0 && size > 1 && value[size - 1] == '\0';
}

static int acquire_lease(void)
{
    char build[64], release[64], model[64], name[128];
    const char *run = getenv("GITHUB_RUN_ID"), *attempt = getenv("GITHUB_RUN_ATTEMPT");
    require(getuid() == 0 && geteuid() == 0 && getgid() == 0 && getegid() == 0 &&
        same_env("GITHUB_ACTIONS", "true") && same_env("RUNNER_ENVIRONMENT", "github-hosted") &&
        same_env("RUNNER_OS", "macOS") && same_env("RUNNER_ARCH", "ARM64") &&
        decimal(run) && decimal(attempt), "expected a privileged GitHub-hosted proof job");
    require(sys_string("kern.osversion", build, sizeof(build)) && strcmp(build, "26A434") == 0 &&
        sys_string("kern.osrelease", release, sizeof(release)) && strcmp(release, "27.0.0") == 0 &&
        sys_string("hw.model", model, sizeof(model)) && strncmp(model, "VirtualMac", 10) == 0,
        "hosted VM image identity differs");
    const int size = snprintf(name, sizeof(name), "ptg-uid-proof-%s-%s", run, attempt);
    require(size > 0 && (size_t)size < sizeof(name), "lease name overflow");
    int parent = open("/private/var/tmp", O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    struct stat st;
    require(parent >= 0 && fstat(parent, &st) == 0 && st.st_uid == 0 &&
        (st.st_mode & S_ISVTX) != 0, "lease parent is not root-owned and sticky");
    /* Each disposable VM admits at most one proof for this fixed UID pair. */
    int pair_lease = openat(parent, "ptg-uid-fork-churn-62081-62082", O_RDWR | O_CREAT | O_EXCL |
        O_NOFOLLOW | O_CLOEXEC, 0600);
    require(pair_lease >= 0 && close(pair_lease) == 0, "could not reserve the fixed worker and peer UID pair");
    int directory = openat(parent, name, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    require(directory >= 0 && fstat(directory, &st) == 0 && st.st_uid == 0 && st.st_gid == 0 &&
        (st.st_mode & 07777) == 0700, "lease directory must be root:wheel 0700");
    int lease = openat(directory, "lease", O_RDWR | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0600);
    require(lease >= 0, "could not acquire the exclusive proof lease");
    (void)close(directory);
    (void)close(parent);
    printf("host build=%s kernel=%s model=%s worker=%u peer=%u\n", build, release, model,
        (unsigned)worker.uid, (unsigned)peer.uid);
    return lease;
}

static bool matching_user(const struct passwd *p, const struct account *a)
{
    return p != NULL && p->pw_uid == a->uid && p->pw_gid == a->gid &&
        strcmp(p->pw_name, a->name) == 0 && strcmp(p->pw_dir, "/var/empty") == 0 &&
        strcmp(p->pw_shell, "/usr/bin/false") == 0;
}

static bool matching_group(const struct group *g, const struct account *a)
{
    return g != NULL && g->gr_gid == a->gid && strcmp(g->gr_name, a->name) == 0 &&
        g->gr_mem != NULL && (g->gr_mem[0] == NULL ||
        (strcmp(g->gr_mem[0], a->name) == 0 && g->gr_mem[1] == NULL));
}

static void check_account(const struct account *a)
{
    require(a->uid > 0 && a->gid > 0 && matching_user(getpwuid(a->uid), a) &&
        matching_user(getpwnam(a->name), a) && matching_group(getgrgid(a->gid), a) &&
        matching_group(getgrnam(a->name), a), "dedicated account or primary group differs");
    uuid_t user, group;
    require(mbr_uid_to_uuid(a->uid, user) == 0, "dedicated membership identity unavailable");
    const gid_t privileged_groups[] = {0, 80}; /* wheel and admin on the pinned hosted image */
    for (size_t i = 0; i < sizeof(privileged_groups) / sizeof(privileged_groups[0]); ++i) {
        int member = -1;
        require(mbr_gid_to_uuid(privileged_groups[i], group) == 0 &&
            mbr_check_membership(user, group, &member) == 0 && member == 0,
            "dedicated account administrative membership could not be excluded");
    }
}

static bool exact_creds(const struct proc_bsdinfo *info, const struct account *a)
{
    return info->pbi_uid == a->uid && info->pbi_ruid == a->uid && info->pbi_svuid == a->uid &&
        info->pbi_gid == a->gid && info->pbi_rgid == a->gid && info->pbi_svgid == a->gid;
}

static bool proc_info(pid_t pid, struct proc_bsdinfo *info)
{
    return pid > 1 && proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, info, sizeof(*info)) == (int)sizeof(*info) &&
        info->pbi_pid == (uint32_t)pid && info->pbi_start_tvsec != 0;
}

static void close_child_fds(int keep)
{
    struct proc_fdinfo fds[128];
    int bytes = proc_pidinfo(getpid(), PROC_PIDLISTFDS, 0, fds, sizeof(fds));
    if (bytes <= 0 || bytes >= (int)sizeof(fds) || bytes % (int)sizeof(fds[0]) != 0)
        _exit(70);
    for (int i = 0; i < bytes / (int)sizeof(fds[0]); ++i) {
        if (fds[i].proc_fd != keep && close(fds[i].proc_fd) != 0)
            _exit(71);
    }
}

static bool exact_snapshot_creds(const struct kinfo_proc *row, const struct account *a)
{
    const struct _pcred *p = &row->kp_eproc.e_pcred;
    const struct _ucred *u = &row->kp_eproc.e_ucred;
    return p->p_ruid == a->uid && p->p_svuid == a->uid && u->cr_uid == a->uid &&
        p->p_rgid == a->gid && p->p_svgid == a->gid &&
        u->cr_ngroups == 1 && u->cr_groups[0] == a->gid;
}

static void drop(const struct account *a, int keep)
{
    alarm(12);
    close_child_fds(keep);
    gid_t primary_group = a->gid;
    if (setgid(a->gid) != 0 || setgroups(1, &primary_group) != 0 || setuid(a->uid) != 0)
        _exit(72);
    const pid_t pid = getpid();
    struct proc_bsdinfo self;
    if (getuid() != a->uid || geteuid() != a->uid || getgid() != a->gid || getegid() != a->gid)
        _exit(73);
    int mib[] = {CTL_KERN, KERN_PROC, KERN_PROC_PID, pid};
    struct kinfo_proc row;
    size_t bytes = sizeof(row);
    if (sysctl(mib, 4, &row, &bytes, NULL, 0) != 0 || bytes != sizeof(row) || row.kp_proc.p_pid != pid)
        _exit(92);
    if (!exact_snapshot_creds(&row, a))
        _exit(90);
    if (!proc_info(pid, &self) || !exact_creds(&self, a))
        _exit(91);
    if (seteuid(0) != -1 || errno != EPERM || setegid(0) != -1 || errno != EPERM)
        _exit(74);
}

/* Inspect real, effective and saved UID, including hybrid identities a broadcast could reach. */
static int population(const struct account *a, pid_t expected)
{
    int mib[] = {CTL_KERN, KERN_PROC, KERN_PROC_ALL};
    size_t bytes = 0;
    if (sysctl(mib, 3, NULL, &bytes, NULL, 0) != 0 || bytes == 0 || bytes > 16 * 1024 * 1024)
        return -1;
    bytes += 128 * sizeof(struct kinfo_proc);
    struct kinfo_proc *rows = malloc(bytes);
    if (rows == NULL)
        return -1;
    int count = 0;
    if (sysctl(mib, 3, rows, &bytes, NULL, 0) != 0 || bytes == 0 || bytes % sizeof(*rows) != 0) {
        count = -1;
    } else {
        for (size_t i = 0; i < bytes / sizeof(*rows); ++i) {
            const struct _pcred *p = &rows[i].kp_eproc.e_pcred;
            const struct _ucred *u = &rows[i].kp_eproc.e_ucred;
            if (p->p_ruid != a->uid && p->p_svuid != a->uid && u->cr_uid != a->uid)
                continue;
            if (!exact_snapshot_creds(&rows[i], a) ||
                (expected > 1 && rows[i].kp_proc.p_pid != expected)) {
                count = -1;
                break;
            }
            ++count;
        }
    }
    free(rows);
    return count;
}

static bool empty_until(const struct account *a)
{
    const int64_t end = now_ms() + step_ms;
    do {
        int count = population(a, 0);
        if (count <= 0)
            return count == 0;
        nap();
    } while (now_ms() < end);
    return false;
}

static bool reap_until(pid_t child, int *status, int64_t end)
{
    do {
        pid_t result = waitpid(child, status, WNOHANG);
        if (result == child)
            return true;
        if (result < 0 && errno != EINTR)
            return false;
        nap();
    } while (now_ms() < end);
    return false;
}

static bool reaped_kill(pid_t child, int64_t end)
{
    int status;
    return reap_until(child, &status, end) && WIFSIGNALED(status) && WTERMSIG(status) == SIGKILL;
}

static bool kill_owned_child(pid_t child)
{
    int status;
    /* This parent has one thread and no SIGCHLD handler or other reaper. */
    return child > 1 && waitpid(child, &status, WNOHANG) == 0 &&
        kill(child, SIGKILL) == 0 && reaped_kill(child, now_ms() + step_ms);
}

static bool sweep_until(const struct account *a, pid_t protected_peer,
    _Atomic int64_t *enter, int64_t end)
{
    if (now_ms() >= end)
        return false;
    pid_t child = fork();
    if (child < 0)
        return false;
    if (child == 0) {
        drop(a, -1);
        if (protected_peer > 1 && (kill(protected_peer, 0) != -1 || errno != EPERM))
            _exit(75);
        /* The Darwin UNIX03 broadcast includes this permanently unprivileged caller. */
        if (enter != NULL) {
            struct timespec t;
            if (clock_gettime(CLOCK_MONOTONIC, &t) != 0)
                _exit(99);
            atomic_store_explicit(enter, t.tv_sec * INT64_C(1000000000) + t.tv_nsec,
                memory_order_release);
        }
        (void)kill(-1, SIGKILL);
        _exit(76);
    }
    int status = 0;
    const bool reaped = reap_until(child, &status, end);
    if (reaped && WIFSIGNALED(status) && WTERMSIG(status) == SIGKILL)
        return true;
    fprintf(stderr, "uid-proof: sweep uid=%u reaped=%d status=%d exited=%d signal=%d\n",
        (unsigned)a->uid, reaped, status, reaped && WIFEXITED(status) ? WEXITSTATUS(status) : -1,
        reaped && WIFSIGNALED(status) ? WTERMSIG(status) : -1);
    /* Do not signal a possibly reaped or recycled PID. The helper also self-expires. */
    return false;
}

struct marker {
    pid_t pid;
    pid_t session;
};

static struct marker read_marker(int fd)
{
    struct marker ready;
    struct pollfd pending = {fd, POLLIN, 0};
    require(poll(&pending, 1, step_ms) == 1 && (pending.revents & POLLIN) != 0 &&
        read(fd, &ready, sizeof(ready)) == (ssize_t)sizeof(ready), "missing bounded fixture readiness");
    require(close(fd) == 0 && ready.pid > 1 && ready.session > 1, "fixture readiness identity invalid");
    return ready;
}

static void publish_marker(int fd)
{
    const struct marker ready = {getpid(), getsid(0)};
    if (write(fd, &ready, sizeof(ready)) != (ssize_t)sizeof(ready) || close(fd) != 0)
        _exit(82);
}

static struct proc_bsdinfo checked_info(pid_t pid, const struct account *a)
{
    struct proc_bsdinfo info;
    require(proc_info(pid, &info) && exact_creds(&info, a) && info.pbi_status != SZOMB,
        "fixture process identity or credentials differ");
    return info;
}

static bool same_identity(const struct proc_bsdinfo *left, const struct proc_bsdinfo *right)
{
    return left->pbi_pid == right->pbi_pid && left->pbi_start_tvsec == right->pbi_start_tvsec &&
        left->pbi_start_tvusec == right->pbi_start_tvusec;
}


enum witness_result { witness_empty, witness_busy, witness_error };
enum { witness_refused = 20, producers = 4, attempts = 16384, fixture_limit = 16 };

static int64_t now_ns(void)
{
    struct timespec t;
    require(clock_gettime(CLOCK_MONOTONIC, &t) == 0, "monotonic clock unavailable");
    return t.tv_sec * INT64_C(1000000000) + t.tv_nsec;
}

static int64_t step_end(int64_t end)
{
    const int64_t local = now_ms() + step_ms;
    return local < end ? local : end;
}

static enum witness_result witness(const struct account *a, int64_t end)
{
    if (now_ms() >= end)
        return witness_error;
    const pid_t verifier = fork();
    if (verifier < 0)
        return witness_error;
    if (verifier == 0) {
        drop(a, -1);
        const struct rlimit limit = {2, 2};
        struct rlimit actual;
        if (setrlimit(RLIMIT_NPROC, &limit) != 0 || getrlimit(RLIMIT_NPROC, &actual) != 0 ||
            actual.rlim_cur != 2 || actual.rlim_max != 2)
            _exit(77);
        const pid_t held = fork();
        if (held == 0) {
            alarm(3);
            (void)pause();
            _exit(78);
        }
        if (held < 0)
            _exit(errno == EAGAIN ? witness_refused : 79);
        const pid_t overflow = fork();
        if (overflow == 0)
            _exit(80);
        const bool refused = overflow == -1 && errno == EAGAIN;
        int status;
        if (overflow > 0)
            (void)reap_until(overflow, &status, step_end(end));
        const bool cleaned = kill_owned_child(held);
        _exit(refused && cleaned ? 0 : 81);
    }
    int status;
    if (!reap_until(verifier, &status, step_end(end)) || !WIFEXITED(status))
        return witness_error;
    if (WEXITSTATUS(status) == 0)
        return witness_empty;
    return WEXITSTATUS(status) == witness_refused ? witness_busy : witness_error;
}

static void check_witness_controls(void)
{
    require(witness(&worker, now_ms() + step_ms) == witness_empty,
        "empty worker baseline did not admit the witness fork");
    int channel[2];
    require(pipe(channel) == 0, "could not create sentinel readiness pipe");
    const pid_t sentinel = fork();
    require(sentinel >= 0, "could not fork live worker sentinel");
    if (sentinel == 0) {
        drop(&worker, channel[1]);
        publish_marker(channel[1]);
        (void)pause();
        _exit(88);
    }
    require(close(channel[1]) == 0 && read_marker(channel[0]).pid == sentinel,
        "live sentinel readiness identity differs");
    const struct proc_bsdinfo before = checked_info(sentinel, &worker);
    require(population(&worker, sentinel) == 1, "live sentinel was not alone before the witness");
    require(witness(&worker, now_ms() + step_ms) == witness_busy,
        "live worker sentinel did not block the witness fork");
    const struct proc_bsdinfo after = checked_info(sentinel, &worker);
    require(same_identity(&before, &after) && kill_owned_child(sentinel) &&
        witness(&worker, now_ms() + step_ms) == witness_empty,
        "sentinel identity, cleanup or empty baseline failed");
    puts("controls live_worker_witness_eagain=1 empty_worker_witness_fork=1");
}

struct churn_trace {
    _Atomic int64_t sweep_enter;
    _Atomic int64_t fork_started[producers][attempts];
    _Atomic int64_t fork_completed[producers][attempts];
};

/* Shared memory is fixture telemetry, never an authenticated workload interface. */
static _Noreturn void churn(struct churn_trace *trace, unsigned producer)
{
    const struct timespec delay = {0, 1000000};
    for (unsigned attempt = 0; attempt < attempts; ++attempt) {
        const int64_t started = now_ns();
        const pid_t child = fork();
        if (child == 0) {
            alarm(12);
            (void)nanosleep(&delay, NULL);
            _exit(0);
        }
        if (child < 0) {
            if (errno != EAGAIN)
                _exit(93);
            (void)nanosleep(&delay, NULL);
            continue;
        }
        const int64_t completed = now_ns();
        atomic_store_explicit(&trace->fork_started[producer][attempt], started, memory_order_relaxed);
        atomic_store_explicit(&trace->fork_completed[producer][attempt], completed, memory_order_release);
        int status;
        pid_t reaped;
        do {
            reaped = waitpid(child, &status, 0);
        } while (reaped < 0 && errno == EINTR);
        if (reaped != child || !((WIFEXITED(status) && WEXITSTATUS(status) == 0) ||
            (WIFSIGNALED(status) && WTERMSIG(status) == SIGKILL)))
            _exit(94);
        /* Keep the attempt budget from naturally draining before the scored deadline. */
        (void)nanosleep(&delay, NULL);
    }
    _exit(95);
}

/* A start and successful parent return inside these bounds establish user-space
 * overlap only. The kernel's internal broadcast PID snapshot is unobserved.
 */
static bool observed_fork(const struct churn_trace *trace, int64_t after, int64_t before,
    int64_t *recorded_start, int64_t *recorded_end)
{
    for (unsigned producer = 0; producer < producers; ++producer) {
        for (unsigned attempt = 0; attempt < attempts; ++attempt) {
            const int64_t completed = atomic_load_explicit(&trace->fork_completed[producer][attempt], memory_order_acquire);
            const int64_t started = atomic_load_explicit(&trace->fork_started[producer][attempt], memory_order_relaxed);
            if (started > after && completed >= started && completed <= before) {
                if (recorded_start != NULL)
                    *recorded_start = started;
                if (recorded_end != NULL)
                    *recorded_end = completed;
                return true;
            }
        }
    }
    return false;
}

static bool verify_churn(unsigned round, pid_t sibling, const struct proc_bsdinfo *sibling_before)
{
    int marker[2];
    struct churn_trace *trace = mmap(NULL, sizeof(*trace), PROT_READ | PROT_WRITE,
        MAP_ANON | MAP_SHARED, -1, 0);
    require(trace != MAP_FAILED && pipe(marker) == 0, "could not allocate churn readiness and telemetry");
    atomic_init(&trace->sweep_enter, 0);
    for (unsigned producer = 0; producer < producers; ++producer) {
        for (unsigned attempt = 0; attempt < attempts; ++attempt) {
            atomic_init(&trace->fork_started[producer][attempt], 0);
            atomic_init(&trace->fork_completed[producer][attempt], 0);
        }
    }
    require(atomic_is_lock_free(&trace->sweep_enter) &&
        atomic_is_lock_free(&trace->fork_started[0][0]) && atomic_is_lock_free(&trace->fork_completed[0][0]),
        "shared 64-bit telemetry atomics are not lock-free");

    const pid_t bootstrap = fork();
    require(bootstrap >= 0, "could not fork churn bootstrap");
    if (bootstrap == 0) {
        drop(&worker, marker[1]);
        const struct rlimit limit = {fixture_limit, fixture_limit};
        struct rlimit actual;
        if (setrlimit(RLIMIT_NPROC, &limit) != 0 || getrlimit(RLIMIT_NPROC, &actual) != 0 ||
            actual.rlim_cur != fixture_limit || actual.rlim_max != fixture_limit)
            _exit(96);
        const pid_t session = fork();
        if (session < 0)
            _exit(83);
        if (session > 0)
            _exit(0);
        alarm(12);
        if (setsid() != getpid())
            _exit(84);
        const pid_t detached = fork();
        if (detached < 0)
            _exit(85);
        if (detached > 0)
            _exit(0);
        alarm(12);
        for (unsigned producer = 1; producer < producers; ++producer) {
            const pid_t seed = fork();
            if (seed < 0)
                _exit(97);
            if (seed == 0) {
                alarm(12);
                if (close(marker[1]) != 0)
                    _exit(98);
                churn(trace, producer);
            }
        }
        publish_marker(marker[1]);
        churn(trace, 0);
    }
    require(close(marker[1]) == 0, "could not release churn marker writer");
    const struct marker ready = read_marker(marker[0]);
    int status;
    require(reap_until(bootstrap, &status, now_ms() + step_ms) &&
        WIFEXITED(status) && WEXITSTATUS(status) == 0, "churn bootstrap did not exit normally");
    const int64_t arm_end = now_ms() + step_ms;
    struct proc_bsdinfo anchor = checked_info(ready.pid, &worker);
    while ((anchor.pbi_ppid != 1 || !observed_fork(trace, 0, now_ns(), NULL, NULL)) && now_ms() < arm_end) {
        nap();
        anchor = checked_info(ready.pid, &worker);
    }
    require(anchor.pbi_ppid == 1 && ready.pid != ready.session && ready.session != getsid(0) &&
        getsid(ready.pid) == ready.session && getpgid(ready.pid) == ready.session &&
        observed_fork(trace, 0, now_ns(), NULL, NULL), "orphaned active churn was not observed");

    const int queue = kqueue();
    struct kevent event = {0};
    const struct timespec nonblocking = {0, 0};
    require(queue >= 0, "could not open churn kqueue");
    EV_SET(&event, (uintptr_t)ready.pid, EVFILT_PROC, EV_ADD | EV_ENABLE | EV_ONESHOT,
        NOTE_EXIT | NOTE_EXITSTATUS, 0, NULL);
    require(kevent(queue, &event, 1, NULL, 0, &nonblocking) == 0,
        "could not register churn anchor exit");
    const struct proc_bsdinfo held = checked_info(ready.pid, &worker);
    require(same_identity(&anchor, &held) && kevent(queue, NULL, 0, &event, 1, &nonblocking) == 0,
        "churn anchor changed or exited before teardown");

    const int64_t end = now_ms() + step_ms;
    const int64_t first_dispatched = now_ns();
    require(sweep_until(&worker, sibling, &trace->sweep_enter, end), "first churn sweep failed");
    const int64_t first_reaped = now_ns();
    const int64_t first_enter = atomic_load_explicit(&trace->sweep_enter, memory_order_acquire);
    int64_t fork_started = 0, fork_completed = 0;
    const bool overlap = first_enter > first_dispatched && first_enter <= first_reaped &&
        observed_fork(trace, first_dispatched, first_reaped, &fork_started, &fork_completed);
    const bool overlap_after_enter = first_enter > 0 && observed_fork(trace, first_enter, first_reaped, NULL, NULL);
    /* There is deliberately no population snapshot before the first witness. */
    enum witness_result result = witness(&worker, end);
    unsigned passes = 1;
    while (result == witness_busy && now_ms() < end) {
        ++passes;
        require(sweep_until(&worker, sibling, NULL, end), "repeated churn sweep failed");
        result = witness(&worker, end);
    }
    require(result == witness_empty && now_ms() < end, "no independent empty witness before churn deadline");
    {
        const int64_t remaining = end - now_ms();
        const struct timespec wait = {remaining > 0 ? remaining / 1000 : 0,
            remaining > 0 ? (remaining % 1000) * 1000000 : 0};
        const bool saw_anchor = kevent(queue, NULL, 0, &event, 1, &wait) == 1 &&
            event.filter == EVFILT_PROC && event.ident == (uintptr_t)ready.pid &&
            (event.flags & EV_ERROR) == 0 && (event.fflags & (NOTE_EXIT | NOTE_EXITSTATUS)) ==
            (NOTE_EXIT | NOTE_EXITSTATUS);
        int exit_status = (int)event.data;
        require(saw_anchor && WIFSIGNALED(exit_status) && WTERMSIG(exit_status) == SIGKILL,
            "churn anchor did not produce its exact native SIGKILL exit");
        require(saw_anchor && close(queue) == 0 && population(&worker, 0) == 0,
            "churn anchor SIGKILL or post-witness worker drain not observed");
    }
    const struct proc_bsdinfo sibling_after = checked_info(sibling, &peer);
    require(same_identity(sibling_before, &sibling_after) && waitpid(sibling, &status, WNOHANG) == 0 &&
        population(&peer, sibling) == 1, "different-UID peer did not survive churn teardown");
    require(munmap(trace, sizeof(*trace)) == 0, "could not release churn telemetry");
    printf("round=%u anchor_pid=%d start=%" PRIu64 ".%06" PRIu64
        " session=%d peer_pid=%d exit_signal=%d sweep_passes=%u worker_empty=1"
        " overlap=%d overlap_after_enter=%d first_sweep_dispatch_ns=%" PRId64
        " first_sweep_enter_ns=%" PRId64 " first_sweep_reaped_ns=%" PRId64
        " fork_start_ns=%" PRId64 " fork_return_ns=%" PRId64 "\n",
        round, ready.pid, held.pbi_start_tvsec, held.pbi_start_tvusec, ready.session, sibling,
        SIGKILL, passes, overlap, overlap_after_enter, first_dispatched, first_enter, first_reaped, fork_started, fork_completed);
    return overlap;
}

static _Noreturn void controller(void)
{
    alarm(controller_seconds);
    /* Empty positive test proves this Darwin libc broadcast includes the dropped caller. */
    require(population(&worker, 0) == 0, "initial worker UID was not empty");
    require(sweep_until(&worker, 0, NULL, now_ms() + step_ms), "sacrificial broadcast did not terminate as expected");
    require(empty_until(&worker), "worker UID did not drain after the sacrificial broadcast");
    check_witness_controls();
    int marker[2];
    require(pipe(marker) == 0, "could not create peer readiness channel");
    pid_t sibling = fork();
    require(sibling >= 0, "could not fork the different-UID peer");
    if (sibling == 0) {
        drop(&peer, marker[1]);
        alarm(controller_seconds + 5);
        publish_marker(marker[1]);
        (void)pause();
        _exit(87);
    }
    require(close(marker[1]) == 0 && read_marker(marker[0]).pid == sibling, "peer readiness identity differs");
    const struct proc_bsdinfo sibling_before = checked_info(sibling, &peer);
    require(population(&peer, sibling) == 1, "peer UID held an unexpected process");
    unsigned overlap_rounds = 0;
    for (unsigned round = 1; round <= rounds; ++round) {
        if (verify_churn(round, sibling, &sibling_before))
            ++overlap_rounds;
    }
    require(kill_owned_child(sibling) && empty_until(&peer),
        "could not reap the owned peer");
    printf("churn_summary completed=%u overlap_rounds=%u worker_empty=1 peer_empty=1\n",
        (unsigned)rounds, overlap_rounds);
    require(overlap_rounds == rounds, "successful fork overlap was not observed in every round");
    _exit(0);
}

static bool recover(const struct account *a)
{
    const int64_t end = now_ms() + 5000;
    do {
        if (!sweep_until(a, 0, NULL, step_end(end)))
            return false;
        const enum witness_result result = witness(a, end);
        if (result == witness_empty)
            return population(a, 0) == 0;
        if (result != witness_busy)
            return false;
    } while (now_ms() < end);
    return false;
}

int main(int argc, char **argv)
{
    (void)argv;
    require(argc == 1, "no user-supplied payload or parameters accepted");
    require(setvbuf(stdout, NULL, _IONBF, 0) == 0 && signal(SIGALRM, SIG_DFL) != SIG_ERR &&
        signal(SIGCHLD, SIG_DFL) != SIG_ERR, "could not set proof process state");
    sigset_t unblocked;
    require(sigemptyset(&unblocked) == 0 && sigprocmask(SIG_SETMASK, &unblocked, NULL) == 0,
        "could not unblock proof deadline signals");
    const int64_t preflight_end = now_ms() + 5000;
    alarm(backup_seconds);
    int lease = acquire_lease();
    check_account(&worker);
    check_account(&peer);
    require(worker.uid != peer.uid && worker.gid != peer.gid && population(&worker, 0) == 0 &&
        population(&peer, 0) == 0, "UIDs/GIDs are not distinct, empty and dedicated");
    require(now_ms() < preflight_end, "preflight would consume root recovery reserve");
    pid_t child = fork();
    require(child >= 0, "could not fork the supervised root controller");
    if (child == 0)
        controller();

    int status;
    const bool reaped = reap_until(child, &status, now_ms() + (controller_seconds + 2) * 1000);
    const bool passed = reaped && WIFEXITED(status) && WEXITSTATUS(status) == 0 &&
        population(&worker, 0) == 0 && population(&peer, 0) == 0;
    if (!passed) {
        if (!reaped)
            (void)kill_owned_child(child);
        bool worker_clean = recover(&worker);
        bool peer_clean = recover(&peer);
        fprintf(stderr, "uid-proof: failed, backup_worker_clean=%d backup_peer_clean=%d\n", worker_clean, peer_clean);
        return 1;
    }
    require(close(lease) == 0, "could not close the owned lease");
    puts("PASS churn_rounds=20 overlap_rounds=20 worker_empty=1 peer_empty=1 root_supervisor_reaped_controller=1");
    return 0;
}
