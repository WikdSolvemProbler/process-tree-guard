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
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/event.h>
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

static void drop(const struct account *a, int keep)
{
    alarm(12);
    close_child_fds(keep);
    if (setgroups(0, NULL) != 0 || setgid(a->gid) != 0 || setuid(a->uid) != 0)
        _exit(72);
    gid_t group = 0;
    struct proc_bsdinfo self;
    if (getuid() != a->uid || geteuid() != a->uid || getgid() != a->gid || getegid() != a->gid ||
        getgroups(1, &group) != 1 || group != a->gid || !proc_info(getpid(), &self) || !exact_creds(&self, a))
        _exit(73);
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
            if (p->p_ruid != a->uid || p->p_svuid != a->uid || u->cr_uid != a->uid ||
                p->p_rgid != a->gid || p->p_svgid != a->gid || u->cr_ngroups != 1 || u->cr_groups[0] != a->gid ||
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

static bool sweep(const struct account *a, pid_t protected_peer)
{
    pid_t child = fork();
    if (child < 0)
        return false;
    if (child == 0) {
        drop(a, -1);
        if (protected_peer > 1 && (kill(protected_peer, 0) != -1 || errno != EPERM))
            _exit(75);
        /* The Darwin UNIX03 broadcast includes this permanently unprivileged caller. */
        (void)kill(-1, SIGKILL);
        _exit(76);
    }
    if (reaped_kill(child, now_ms() + step_ms))
        return true;
    /* Do not signal a possibly reaped or recycled PID. The helper also self-expires. */
    return false;
}

static void verify_nproc(void)
{
    require(population(&worker, 0) == 0, "worker UID was populated before the verifier");
    pid_t verifier = fork();
    require(verifier >= 0, "could not fork the worker UID verifier");
    if (verifier == 0) {
        drop(&worker, -1);
        const struct rlimit limit = {2, 2};
        struct rlimit actual;
        if (setrlimit(RLIMIT_NPROC, &limit) != 0 || getrlimit(RLIMIT_NPROC, &actual) != 0 ||
            actual.rlim_cur != 2 || actual.rlim_max != 2)
            _exit(77);
        pid_t held = fork();
        if (held == 0) {
            alarm(3);
            (void)pause();
            _exit(78);
        }
        if (held < 0)
            _exit(79);
        pid_t overflow = fork();
        if (overflow == 0)
            _exit(80);
        bool refused = overflow == -1 && errno == EAGAIN;
        int status;
        if (overflow > 0)
            (void)reap_until(overflow, &status, now_ms() + step_ms);
        bool cleaned = kill_owned_child(held);
        _exit(refused && cleaned ? 0 : 81);
    }
    int status;
    require(reap_until(verifier, &status, now_ms() + step_ms) && WIFEXITED(status) && WEXITSTATUS(status) == 0 &&
        empty_until(&worker), "RLIMIT_NPROC=2 positive/negative UID capacity oracle failed");
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

static void verify_escape(unsigned round, pid_t sibling, const struct proc_bsdinfo *sibling_before)
{
    int marker[2];
    require(population(&worker, 0) == 0 && pipe(marker) == 0, "worker UID or readiness channel not clean");
    pid_t parent = fork();
    require(parent >= 0, "could not fork the escape fixture");
    if (parent == 0) {
        drop(&worker, marker[1]);
        pid_t session = fork();
        if (session < 0)
            _exit(83);
        if (session > 0)
            _exit(0);
        alarm(12);
        if (setsid() != getpid())
            _exit(84);
        pid_t detached = fork();
        if (detached < 0)
            _exit(85);
        if (detached > 0)
            _exit(0);
        alarm(12);
        publish_marker(marker[1]);
        (void)pause();
        _exit(86);
    }
    require(close(marker[1]) == 0, "could not release controller marker writer");
    const struct marker ready = read_marker(marker[0]);
    int status;
    require(reap_until(parent, &status, now_ms() + step_ms) && WIFEXITED(status) && WEXITSTATUS(status) == 0,
        "initial fixture child was not reaped normally");
    const int64_t end = now_ms() + step_ms;
    struct proc_bsdinfo before = checked_info(ready.pid, &worker);
    int count = population(&worker, ready.pid);
    while ((before.pbi_ppid != 1 || count != 1) && now_ms() < end) {
        nap();
        before = checked_info(ready.pid, &worker);
        count = population(&worker, ready.pid);
    }
    require(before.pbi_ppid == 1 && ready.pid != ready.session && ready.session != getsid(0) &&
        getsid(ready.pid) == ready.session && getpgid(ready.pid) == ready.session &&
        count == 1, "orphaned session escape or positive UID population not proven");
    int queue = kqueue();
    struct kevent event;
    const struct timespec nonblocking = {0, 0};
    require(queue >= 0, "could not open kqueue");
    EV_SET(&event, (uintptr_t)ready.pid, EVFILT_PROC, EV_ADD | EV_ENABLE | EV_ONESHOT,
        NOTE_EXIT | NOTE_EXITSTATUS, 0, NULL);
    require(kevent(queue, &event, 1, NULL, 0, &nonblocking) == 0, "could not register held exit observation");
    const struct proc_bsdinfo held = checked_info(ready.pid, &worker);
    require(same_identity(&before, &held) && kevent(queue, NULL, 0, &event, 1, &nonblocking) == 0,
        "worker identity changed or exited before the sweep");
    require(sweep(&worker, sibling), "the UID-only sweeper did not terminate by SIGKILL");
    const struct timespec wait = {2, 500000000};
    require(kevent(queue, NULL, 0, &event, 1, &wait) == 1 && event.ident == (uintptr_t)ready.pid &&
        event.filter == EVFILT_PROC && (event.flags & EV_ERROR) == 0 &&
        (event.fflags & (NOTE_EXIT | NOTE_EXITSTATUS)) == (NOTE_EXIT | NOTE_EXITSTATUS),
        "held escape process did not produce its native exit event");
    int exit_status = (int)event.data;
    require(WIFSIGNALED(exit_status) && WTERMSIG(exit_status) == SIGKILL,
        "held escape process did not terminate by SIGKILL");
    require(close(queue) == 0 && empty_until(&worker), "worker UID did not drain");
    const struct proc_bsdinfo sibling_after = checked_info(sibling, &peer);
    require(same_identity(sibling_before, &sibling_after) && waitpid(sibling, &status, WNOHANG) == 0 &&
        population(&peer, sibling) == 1, "different-UID peer did not survive the sweep");
    verify_nproc();
    printf("round=%u escaped_pid=%d start=%" PRIu64 ".%06" PRIu64 " session=%d peer_pid=%d exit_signal=%d worker_empty=1 nproc=2\n",
        round, ready.pid, held.pbi_start_tvsec, held.pbi_start_tvusec, ready.session, sibling, SIGKILL);
}

static _Noreturn void controller(void)
{
    alarm(controller_seconds);
    /* Empty positive test proves this Darwin libc broadcast includes the dropped caller. */
    require(population(&worker, 0) == 0 && sweep(&worker, 0) && empty_until(&worker),
        "sacrificial broadcast or initial UID emptiness proof failed");
    verify_nproc();
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
    for (unsigned round = 1; round <= rounds; ++round)
        verify_escape(round, sibling, &sibling_before);
    require(kill_owned_child(sibling) && empty_until(&peer),
        "could not reap the owned peer");
    _exit(0);
}

static bool recover(const struct account *a)
{
    int count = population(a, 0);
    return count == 0 || (count > 0 && sweep(a, 0) && empty_until(a));
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
    puts("PASS rounds=20 worker_empty=1 peer_empty=1 root_supervisor_reaped_controller=1");
    return 0;
}
