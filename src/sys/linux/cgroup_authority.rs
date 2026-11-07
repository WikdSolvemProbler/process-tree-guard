// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

use super::*;
use std::ffi::{CStr, CString};

const CGROUP2_MAGIC: i64 = 0x6367_7270;
const AT_FDCWD: c_int = -100;
const AT_SYMLINK_NOFOLLOW: c_int = 0x100;
const AT_EMPTY_PATH: c_int = 0x1000;
const O_CLOEXEC: c_int = 0o2_000_000;
// Linux ARM64 overrides these two asm-generic open flags.
#[cfg(target_arch = "aarch64")]
const O_NOFOLLOW: c_int = 0o100_000;
#[cfg(target_arch = "aarch64")]
const O_DIRECTORY: c_int = 0o40_000;
#[cfg(target_arch = "x86_64")]
const O_NOFOLLOW: c_int = 0o400_000;
#[cfg(target_arch = "x86_64")]
const O_DIRECTORY: c_int = 0o200_000;
#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
compile_error!("cgroup authority has pinned Linux ARM64 and x86-64 ABIs only");
const O_PATH: c_int = 0o10_000_000;
const O_NONBLOCK: c_int = 0o4000;
const O_WRONLY: c_int = 1;
const S_IFMT: u16 = 0o170_000;
const S_IFDIR: u16 = 0o040_000;
const S_IFREG: u16 = 0o100_000;
#[cfg(test)]
const S_IFLNK: u16 = 0o120_000;
const STATX_BASIC_STATS: u32 = 0x07ff;
const STATX_MNT_ID: u32 = 0x1000;
const REQUIRED_STATX: u32 = 0x011b | STATX_MNT_ID;

#[path = "settlement.rs"]
pub(super) mod settlement;

// Linux v5.14 statx and 64-bit Linux GNU statfs layouts.
#[repr(C)]
#[derive(Default)]
struct StatxTime {
    seconds: i64,
    nanoseconds: u32,
    reserved: i32,
}

#[repr(C, align(8))]
#[derive(Default)]
struct Statx {
    mask: u32,
    block_size: u32,
    attributes: u64,
    links: u32,
    uid: u32,
    gid: u32,
    mode: u16,
    reserved0: u16,
    inode: u64,
    size: u64,
    blocks: u64,
    attributes_mask: u64,
    access: StatxTime,
    birth: StatxTime,
    change: StatxTime,
    modified: StatxTime,
    rdev_major: u32,
    rdev_minor: u32,
    dev_major: u32,
    dev_minor: u32,
    mount_id: u64,
    reserved: [u64; 13],
}

#[repr(C)]
#[derive(Default)]
struct Statfs {
    kind: i64,
    block_size: i64,
    blocks: u64,
    blocks_free: u64,
    blocks_available: u64,
    files: u64,
    files_free: u64,
    fsid: [i32; 2],
    name_length: i64,
    fragment_size: i64,
    flags: i64,
    reserved: [i64; 4],
}

const _: () = {
    assert!(std::mem::size_of::<Statx>() == 256);
    assert!(std::mem::offset_of!(Statx, inode) == 32);
    assert!(std::mem::offset_of!(Statx, mount_id) == 144);
    assert!(std::mem::size_of::<Statfs>() == 120);
};

unsafe extern "C" {
    #[link_name = "openat"]
    fn fd_openat(parent: c_int, name: *const c_char, flags: c_int, ...) -> c_int;
    #[link_name = "mkdirat"]
    fn fd_mkdirat(parent: c_int, name: *const c_char, mode: u32) -> c_int;
    #[link_name = "statx"]
    fn fd_statx(
        parent: c_int,
        name: *const c_char,
        flags: c_int,
        mask: u32,
        out: *mut Statx,
    ) -> c_int;
    #[link_name = "fstatfs"]
    fn fd_statfs(fd: c_int, out: *mut Statfs) -> c_int;
    #[link_name = "read"]
    fn fd_read(fd: c_int, out: *mut c_void, size: usize) -> isize;
    #[link_name = "getrandom"]
    fn random_bytes(out: *mut c_void, size: usize, flags: u32) -> isize;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Identity {
    device: (u32, u32),
    mount: u64,
    inode: u64,
}

impl Identity {
    fn from(stat: &Statx) -> Self {
        Self {
            device: (stat.dev_major, stat.dev_minor),
            mount: stat.mount_id,
            inode: stat.inode,
        }
    }

    fn same_mount(self, other: Self) -> bool {
        self.device == other.device && self.mount == other.mount
    }
}

fn metadata(parent: RawFd, name: &CStr, flags: c_int) -> Probe<Statx> {
    let mut stat = Statx::default();
    native(os_result(unsafe {
        fd_statx(
            parent,
            name.as_ptr(),
            flags,
            STATX_BASIC_STATS | STATX_MNT_ID,
            &mut stat,
        )
    } as isize))?;
    check(
        stat.mask & REQUIRED_STATX == REQUIRED_STATX && stat.inode != 0,
        "authority statx omitted required fields",
    )?;
    Ok(stat)
}

fn descriptor_stat(fd: &OwnedFd) -> Probe<Statx> {
    metadata(fd.as_raw_fd(), c"", AT_EMPTY_PATH | AT_SYMLINK_NOFOLLOW)
}

fn require_root_mode(stat: &Statx, kind: u16) -> Probe<Identity> {
    check(
        stat.uid == 0 && stat.mode & S_IFMT == kind && stat.mode & 0o022 == 0,
        "authority is not a protected root-owned inode",
    )?;
    Ok(Identity::from(stat))
}

fn require_cgroup2(fd: &OwnedFd) -> Probe<()> {
    let mut stat = Statfs::default();
    native(os_result(
        unsafe { fd_statfs(fd.as_raw_fd(), &mut stat) } as isize
    ))?;
    check(stat.kind == CGROUP2_MAGIC, "authority is not on cgroup v2")
}

fn open_relative(parent: RawFd, name: &CStr, flags: c_int, kind: u16) -> Probe<OwnedFd> {
    let fd =
        fd_from_call(unsafe { fd_openat(parent, name.as_ptr(), flags | O_CLOEXEC | O_NOFOLLOW) })?;
    native(close_on_exec(fd.as_raw_fd()))?;
    // O_PATH | O_NOFOLLOW can return a descriptor for the symlink itself.
    // Inspect that held inode before allowing it to cross this boundary.
    check(
        descriptor_stat(&fd)?.mode & S_IFMT == kind,
        "opened authority has an unexpected inode type",
    )?;
    Ok(fd)
}

fn require_name_matches(parent: RawFd, name: &CStr, fd: &OwnedFd) -> Probe<Identity> {
    let held = descriptor_stat(fd)?;
    let named = metadata(parent, name, AT_SYMLINK_NOFOLLOW)?;
    check(
        Identity::from(&held) == Identity::from(&named),
        "authority directory entry changed during open",
    )?;
    Ok(Identity::from(&held))
}

fn directory(parent: RawFd, name: &CStr) -> Probe<OwnedFd> {
    let fd = open_relative(parent, name, O_PATH | O_DIRECTORY, S_IFDIR)?;
    require_root_mode(&descriptor_stat(&fd)?, S_IFDIR)?;
    require_name_matches(parent, name, &fd)?;
    Ok(fd)
}

fn control(directory: &OwnedFd, name: &CStr, write: bool) -> Probe<OwnedFd> {
    let fd = open_relative(
        directory.as_raw_fd(),
        name,
        O_NONBLOCK | if write { O_WRONLY } else { 0 },
        S_IFREG,
    )?;
    require_cgroup2(&fd)?;
    let file = require_root_mode(&descriptor_stat(&fd)?, S_IFREG)?;
    let parent = Identity::from(&descriptor_stat(directory)?);
    check(
        file.same_mount(parent),
        "control mount differs from its leaf",
    )?;
    require_name_matches(directory.as_raw_fd(), name, &fd)?;
    Ok(fd)
}

fn text(fd: &OwnedFd) -> Probe<String> {
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0u8; 256];
        let count = native(os_result(unsafe {
            fd_read(fd.as_raw_fd(), chunk.as_mut_ptr().cast(), chunk.len())
        }))?;
        if count == 0 {
            return String::from_utf8(bytes).map_err(|_| "non-UTF-8 cgroup control".into());
        }
        bytes.extend_from_slice(&chunk[..count]);
        check(bytes.len() <= 4096, "cgroup control exceeds bounded input")?;
    }
}

fn require_domain(directory: &OwnedFd) -> Probe<()> {
    let kind = control(directory, c"cgroup.type", false)?;
    check(
        text(&kind)? == "domain\n",
        "activation requires a domain cgroup",
    )
}

fn require_fresh(directory: &OwnedFd) -> Probe<()> {
    require_domain(directory)?;
    let subtree = control(directory, c"cgroup.subtree_control", false)?;
    check(
        text(&subtree)?.trim().is_empty(),
        "fresh leaf has subtree controllers enabled",
    )?;
    let events = control(directory, c"cgroup.events", false)?;
    let value = text(&events)?;
    check(
        value
            .lines()
            .filter(|line| line.starts_with("populated "))
            .eq(["populated 0"])
            && value
                .lines()
                .filter(|line| line.starts_with("frozen "))
                .eq(["frozen 0"]),
        "fresh leaf is populated or frozen",
    )
}

struct InstallationRoot {
    directory: OwnedFd,
    identity: Identity,
}

// The fixed installation path is chosen by trusted activation code.
// The service must start outside caller-controlled mount and user namespaces.
fn installation_root() -> Probe<InstallationRoot> {
    let root = directory(AT_FDCWD, c"/")?;
    let sys = directory(root.as_raw_fd(), c"sys")?;
    let fs = directory(sys.as_raw_fd(), c"fs")?;
    let hierarchy = directory(fs.as_raw_fd(), c"cgroup")?;
    require_cgroup2(&hierarchy)?;
    let _hierarchy_migration = control(&hierarchy, c"cgroup.procs", false)?;
    let directory = directory(hierarchy.as_raw_fd(), c"process-tree-guard")?;
    require_cgroup2(&directory)?;
    let identity = Identity::from(&descriptor_stat(&directory)?);
    check(
        identity.same_mount(Identity::from(&descriptor_stat(&hierarchy)?)),
        "installation root changes cgroup mounts",
    )?;
    require_domain(&directory)?;
    let _migration = control(&directory, c"cgroup.procs", false)?;
    Ok(InstallationRoot {
        directory,
        identity,
    })
}

fn generated_name() -> Probe<CString> {
    let mut random = [0u8; 16];
    // GRND_NONBLOCK refuses rather than waiting for initial entropy.
    let count = native(os_result(unsafe {
        random_bytes(random.as_mut_ptr().cast(), random.len(), 1)
    }))?;
    check(count == random.len(), "short random leaf name")?;
    let mut name = String::from("run-");
    for byte in random {
        use std::fmt::Write;
        write!(&mut name, "{byte:02x}").map_err(|error| error.to_string())?;
    }
    CString::new(name).map_err(|error| error.to_string())
}

#[must_use = "a created leaf must remain accounted for until verified removal"]
pub(super) struct CreatedLeaf {
    parent: InstallationRoot,
    name: CString,
    // Retain the actual protected directory before later control opens can
    // fail. A mkdir token without this anchor is never enough to remove a name.
    anchor: Option<LeafAnchor>,
}

struct LeafAnchor {
    directory: OwnedFd,
    identity: Identity,
}

pub(super) struct ProvisionFailure {
    pub(super) error: String,
    pub(super) created: Option<CreatedLeaf>,
    // A failed mkdir result is not proof of absence or deletion authority.
    // Retain its real parent/name and raw outcome rather than losing them.
    pub(super) refused_create: Option<RefusedCreate>,
}

pub(super) struct RefusedCreate {
    parent: InstallationRoot,
    name: CString,
    result: c_int,
    errno: Option<c_int>,
}

pub(super) struct LeafProvenance {
    creation: CreatedLeaf,
    identity: Identity,
}

struct VerifiedHandles {
    directory: OwnedFd,
    watchdog_kill: OwnedFd,
    supervisor_kill: OwnedFd,
    events: OwnedFd,
    identity: Identity,
}

fn open_created(created: &mut CreatedLeaf) -> Probe<VerifiedHandles> {
    let parent = &created.parent;
    check(
        Identity::from(&descriptor_stat(&parent.directory)?) == parent.identity,
        "installation root identity changed",
    )?;
    let directory = directory(parent.directory.as_raw_fd(), &created.name)?;
    require_cgroup2(&directory)?;
    let identity = Identity::from(&descriptor_stat(&directory)?);
    check(
        identity.same_mount(parent.identity),
        "fresh leaf changes mounts",
    )?;
    created.anchor = Some(LeafAnchor { directory, identity });
    let anchor = created.anchor.as_ref().ok_or("created leaf anchor missing")?;
    let directory = &anchor.directory;
    require_fresh(&directory)?;
    let _migration = control(&directory, c"cgroup.procs", false)?;
    let watchdog_kill = control(&directory, c"cgroup.kill", true)?;
    let supervisor_kill = control(&directory, c"cgroup.kill", true)?;
    check(
        Identity::from(&descriptor_stat(&watchdog_kill)?)
            == Identity::from(&descriptor_stat(&supervisor_kill)?),
        "independent kill authorities refer to different inodes",
    )?;
    let events = control(&directory, c"cgroup.events", false)?;
    require_name_matches(parent.directory.as_raw_fd(), &created.name, &directory)?;
    let anchor = created.anchor.take().ok_or("verified leaf anchor lost")?;
    Ok(VerifiedHandles {
        directory: anchor.directory,
        watchdog_kill,
        supervisor_kill,
        events,
        identity: anchor.identity,
    })
}

// Errors after mkdir preserve the created-leaf token. Nothing auto-unlinks
// an unverified path, and no failure case can release a payload.
pub(super) fn provision() -> std::result::Result<DomainControls, ProvisionFailure> {
    let before_create = || -> Probe<(InstallationRoot, CString)> {
        check(
            unsafe { geteuid() } == 0,
            "fresh cgroup authority requires trusted root",
        )?;
        let parent = installation_root()?;
        let name = generated_name()?;
        Ok((parent, name))
    };
    let (parent, name) = before_create().map_err(|error| ProvisionFailure {
        error,
        created: None,
        refused_create: None,
    })?;
    let result = unsafe { fd_mkdirat(parent.directory.as_raw_fd(), name.as_ptr(), 0o700) };
    if result != 0 {
        let errno = io::Error::last_os_error().raw_os_error();
        return Err(ProvisionFailure {
            error: "native cgroup mkdir refused; parent/name and raw outcome retained".into(),
            created: None,
            refused_create: Some(RefusedCreate { parent, name, result, errno }),
        });
    }
    let mut created = CreatedLeaf { parent, name, anchor: None };
    match open_created(&mut created) {
        Ok(handles) => Ok(DomainControls {
            gate: None,
            gate_cancel: std::cell::Cell::new(None),
            gate_release: None,
            directory: handles.directory,
            watchdog_kill: handles.watchdog_kill,
            supervisor_kill: handles.supervisor_kill,
            events: handles.events,
            provenance: LeafProvenance {
                creation: created,
                identity: handles.identity,
            },
        }),
        Err(error) => Err(ProvisionFailure {
            error,
            created: Some(created),
            refused_create: None,
        }),
    }
}

impl CreatedLeaf {
    pub(super) fn name(&self) -> &CStr {
        &self.name
    }

    pub(super) fn has_anchor(&self) -> bool { self.anchor.is_some() }

    pub(super) fn validate_unactivated(&self) -> Probe<()> {
        let anchor = self.anchor.as_ref().ok_or("unactivated leaf has no held identity anchor")?;
        let parent = &self.parent.directory;
        require_cgroup2(parent)?;
        check(require_root_mode(&descriptor_stat(parent)?, S_IFDIR)? == self.parent.identity,
              "unactivated leaf parent identity changed")?;
        require_cgroup2(&anchor.directory)?;
        let held = require_root_mode(&descriptor_stat(&anchor.directory)?, S_IFDIR)?;
        check(held == anchor.identity && held.same_mount(self.parent.identity),
              "unactivated held leaf identity or mount changed")?;
        require_name_matches(parent.as_raw_fd(), &self.name, &anchor.directory)?;
        require_fresh(&anchor.directory)
    }
}

impl DomainControls {
    pub(super) fn revalidate(&self) -> Probe<()> {
        let creation = &self.provenance.creation;
        let parent = &creation.parent.directory;
        require_root_mode(&descriptor_stat(parent)?, S_IFDIR)?;
        check(
            Identity::from(&descriptor_stat(parent)?) == creation.parent.identity,
            "retained parent identity changed",
        )?;
        require_cgroup2(&self.directory)?;
        let held = require_root_mode(&descriptor_stat(&self.directory)?, S_IFDIR)?;
        check(
            held == self.provenance.identity,
            "retained leaf identity changed",
        )?;
        require_name_matches(parent.as_raw_fd(), &creation.name, &self.directory)?;
        require_domain(&self.directory)?;
        for (name, fd) in [
            (c"cgroup.kill", &self.watchdog_kill),
            (c"cgroup.kill", &self.supervisor_kill),
            (c"cgroup.events", &self.events),
        ] {
            require_cgroup2(fd)?;
            let file = require_root_mode(&descriptor_stat(fd)?, S_IFREG)?;
            check(file.same_mount(held), "retained control changes mounts")?;
            require_name_matches(self.directory.as_raw_fd(), name, fd)?;
        }
        Ok(())
    }
}
