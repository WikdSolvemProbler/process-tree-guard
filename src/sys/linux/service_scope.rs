// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
//! Held per-invocation service authority. No global payload cgroup is permitted.
//! Entry-origin minting is deliberately absent until actual root-manager origin,
//! original public parent/ancestor capabilities and pre-manager absolute lease
//! are established. Path/env/nonce/PPID1 and an unsafe caller promise cannot mint.
use super::*;
use std::sync::Arc;

// Only this module may construct either type. There is no public/unsafe raw-FD
// constructor and no environment/request-path conversion into a trusted token.
struct CheckedEntryOrigin {
    unit_components: Vec<CString>,
    main_pid: c_int,
    runtime_ns: u64,
    startup_ns: u64,
    stop_ns: u64,
    randomized_extra_ns: u64,
    type_exec: bool,
    delegate: bool,
    subgroup_supervisor: bool,
    recursive_kill: bool,
    send_sigkill: bool,
    kill_signal: c_int,
    final_kill_signal: c_int,
    restart_no: bool,
    notifications_disabled: bool,
}

fn checked_entry_origin() -> Probe<CheckedEntryOrigin> {
    // SUBSTANTIVE ENTRY BARRIER, not a synthetic successful receipt. The next
    // real entry must verify authoritative manager properties/protected exec
    // and bootstrap/namespace identity and retain the ORIGINAL API owner set
    // plus original absolute admission timer BEFORE manager startup. Existing
    // peer-only PTGEX3 authentication is not that public ownership contract.
    Err("trusted root-manager entry, original parent/all-ancestor ownership and original absolute lease are not established".into())
}

struct BoundDirectory { name: CString, directory: OwnedFd, identity: Identity,
    migration:OwnedFd,migration_identity:Identity }

#[must_use="service anchor must remain owned through all actors and outcome/exit"]
pub(in super::super) struct TrustedUnitAnchor {
    hierarchy: OwnedFd,
    hierarchy_identity: Identity,
    hierarchy_migration:OwnedFd,
    hierarchy_migration_identity:Identity,
    ancestry: Vec<BoundDirectory>,
    supervisor: OwnedFd,
    supervisor_identity: Identity,
    supervisor_migration:OwnedFd,
    supervisor_migration_identity:Identity,
    unit_kill: OwnedFd,
    unit_events: OwnedFd,
    origin: CheckedEntryOrigin,
    expected_membership: String,
}

#[must_use="verified service scope is not a request-supplied cgroup path"]
pub(crate) struct VerifiedServiceScope {
    anchor: Arc<TrustedUnitAnchor>,
}

fn bounded_membership() -> Probe<String> {
    use std::io::Read;
    let file=io_result(fs::File::open("/proc/self/cgroup"))?;
    let mut bytes=Vec::new();
    io_result(file.take(4097).read_to_end(&mut bytes))?;
    check(bytes.len()<=4096,"service cgroup membership exceeds bound")?;
    let text=String::from_utf8(bytes).map_err(|_|"service cgroup membership is not UTF-8")?;
    let mut rows=text.lines();
    let row=rows.next().ok_or("service cgroup membership missing")?;
    check(rows.next().is_none(),"service requires one unified cgroup membership")?;
    row.strip_prefix("0::").map(str::to_owned).ok_or_else(||"service is not on unified cgroup membership".into())
}

impl CheckedEntryOrigin {
    fn require_fixed_policy(&self) -> Probe<()> {
        check(self.main_pid>0 && self.main_pid as u32==std::process::id(),"manager MainPID differs from this service")?;
        check(self.runtime_ns>0 && self.startup_ns>0 && self.stop_ns>0
            && self.runtime_ns!=u64::MAX && self.startup_ns!=u64::MAX && self.stop_ns!=u64::MAX,
            "service requires finite runtime/start/stop authority")?;
        check(self.type_exec && self.delegate && self.subgroup_supervisor
            && self.recursive_kill && self.send_sigkill && self.kill_signal==9
            && self.final_kill_signal==9 && self.restart_no
            && self.notifications_disabled && self.randomized_extra_ns==0,
            "service is not the fixed independently bounded recursive-kill policy")?;
        check(!self.unit_components.is_empty() && self.unit_components.len()<=64,"unit path component count exceeds bound or is hierarchy root")?;
        let mut path_bytes=0usize;
        for component in &self.unit_components {
            let value=component.to_bytes();
            check(!value.is_empty() && value.len()<=255 && value!=b"." && value!=b".." && !value.contains(&b'/'),"invalid authoritative unit path component")?;
            path_bytes=path_bytes.checked_add(value.len()+1).filter(|n|*n<=4096).ok_or("unit path byte bound exceeded")?;
        }
        Ok(())
    }
}

impl VerifiedServiceScope {
    pub(in super::super) fn capture_checked_entry() -> Probe<Self> {
        let origin=checked_entry_origin()?;
        Self::open_verified(origin)
    }

    // Private and currently unminted: only a future actual checked entry can
    // supply CheckedEntryOrigin. This is real descriptor-relative validation,
    // never permission to trust a user-provided unit path or property snapshot.
    fn open_verified(origin:CheckedEntryOrigin) -> Probe<Self> {
        check(unsafe{geteuid()}==0,"service scope requires privileged root")?;
        origin.require_fixed_policy()?;
        let root=directory(AT_FDCWD,c"/")?;
        let sys=directory(root.as_raw_fd(),c"sys")?;
        let fs=directory(sys.as_raw_fd(),c"fs")?;
        let hierarchy=directory(fs.as_raw_fd(),c"cgroup")?;
        require_cgroup2(&hierarchy)?;
        let hierarchy_identity=require_root_mode(&descriptor_stat(&hierarchy)?,S_IFDIR)?;
        let hierarchy_migration=control(&hierarchy,c"cgroup.procs",false)?;
        let hierarchy_migration_identity=Identity::from(&descriptor_stat(&hierarchy_migration)?);
        let mut ancestry:Vec<BoundDirectory>=Vec::new();
        let mut expected_membership=String::new();
        for name in &origin.unit_components {
            let parent=ancestry.last().map_or(&hierarchy,|bound|&bound.directory);
            let directory=directory(parent.as_raw_fd(),name)?;
            require_cgroup2(&directory)?;
            let identity=require_root_mode(&descriptor_stat(&directory)?,S_IFDIR)?;
            check(identity.same_mount(hierarchy_identity),"unit ancestry changes cgroup mounts")?;
            expected_membership.push('/');
            expected_membership.push_str(name.to_str().map_err(|_|"unit path is not UTF-8")?);
            let migration=control(&directory,c"cgroup.procs",false)?;
            let migration_identity=Identity::from(&descriptor_stat(&migration)?);
            ancestry.push(BoundDirectory{name:name.clone(),directory,identity,migration,migration_identity});
        }
        let unit=&ancestry.last().ok_or("unit anchor missing")?.directory;
        require_domain(unit)?;
        let supervisor=directory(unit.as_raw_fd(),c"supervisor")?;
        let supervisor_identity=require_root_mode(&descriptor_stat(&supervisor)?,S_IFDIR)?;
        require_cgroup2(&supervisor)?;
        require_domain(&supervisor)?;
        check(supervisor_identity.same_mount(hierarchy_identity),"supervisor subgroup changes mount")?;
        let supervisor_migration=control(&supervisor,c"cgroup.procs",false)?;
        let supervisor_migration_identity=Identity::from(&descriptor_stat(&supervisor_migration)?);
        expected_membership.push_str("/supervisor");
        check(bounded_membership()?==expected_membership,"supervisor is outside the exact managed unit subgroup")?;
        let unit_kill=control(unit,c"cgroup.kill",true)?;
        let unit_events=control(unit,c"cgroup.events",false)?;
        let anchor=Arc::new(TrustedUnitAnchor{hierarchy,hierarchy_identity,hierarchy_migration,hierarchy_migration_identity,ancestry,supervisor,
            supervisor_identity,supervisor_migration,supervisor_migration_identity,unit_kill,unit_events,origin,expected_membership});
        anchor.revalidate()?;
        Ok(Self{anchor})
    }

    pub(in super::super) fn retain_anchor(&self) -> Arc<TrustedUnitAnchor> {Arc::clone(&self.anchor)}
    pub(in super::super::super) fn revalidate(&self) -> Probe<()> {self.anchor.revalidate()}
    pub(in super::super) fn matches_anchor(&self,anchor:&Arc<TrustedUnitAnchor>)->bool {Arc::ptr_eq(&self.anchor,anchor)}
}

impl TrustedUnitAnchor {
    fn unit_directory(&self) -> Probe<&OwnedFd> {
        Ok(&self.ancestry.last().ok_or("held unit directory missing")?.directory)
    }
    pub(in super::super) fn revalidate(&self) -> Probe<()> {
        self.origin.require_fixed_policy()?;
        require_cgroup2(&self.hierarchy)?;
        check(require_root_mode(&descriptor_stat(&self.hierarchy)?,S_IFDIR)?==self.hierarchy_identity,
            "held cgroup hierarchy identity changed")?;
        self.require_migration(&self.hierarchy,&self.hierarchy_migration,self.hierarchy_migration_identity)?;
        let mut parent=&self.hierarchy;
        for bound in &self.ancestry {
            require_cgroup2(&bound.directory)?;
            check(require_root_mode(&descriptor_stat(&bound.directory)?,S_IFDIR)?==bound.identity,
                "held unit ancestor identity/ownership changed")?;
            require_name_matches(parent.as_raw_fd(),&bound.name,&bound.directory)?;
            self.require_migration(&bound.directory,&bound.migration,bound.migration_identity)?;
            parent=&bound.directory;
        }
        let unit=self.unit_directory()?;
        require_domain(unit)?;
        require_cgroup2(&self.supervisor)?;
        require_domain(&self.supervisor)?;
        check(require_root_mode(&descriptor_stat(&self.supervisor)?,S_IFDIR)?==self.supervisor_identity,
            "held supervisor identity/ownership changed")?;
        require_name_matches(unit.as_raw_fd(),c"supervisor",&self.supervisor)?;
        self.require_migration(&self.supervisor,&self.supervisor_migration,self.supervisor_migration_identity)?;
        check(bounded_membership()?==self.expected_membership,"supervisor migrated outside managed unit")?;
        for (name,fd) in [(c"cgroup.kill",&self.unit_kill),(c"cgroup.events",&self.unit_events)] {
            require_cgroup2(fd)?;
            require_root_mode(&descriptor_stat(fd)?,S_IFREG)?;
            require_name_matches(unit.as_raw_fd(),name,fd)?;
        }
        Ok(())
    }
    fn require_migration(&self,parent:&OwnedFd,fd:&OwnedFd,expected:Identity)->Probe<()> {
        require_cgroup2(fd)?;
        let identity=require_root_mode(&descriptor_stat(fd)?,S_IFREG)?;
        check(identity==expected && identity.same_mount(self.hierarchy_identity),"held migration authority identity/mount/ownership changed")?;
        require_name_matches(parent.as_raw_fd(),c"cgroup.procs",fd)?;Ok(())
    }
    pub(super) fn duplicate_leaf_parent(self:&Arc<Self>) -> Probe<InstallationRoot> {
        self.revalidate()?;
        let source=self.unit_directory()?;
        let directory=fd_from_call(unsafe{fcntl(source.as_raw_fd(),F_DUPFD_CLOEXEC,3 as c_int)})?;
        let identity=require_root_mode(&descriptor_stat(&directory)?,S_IFDIR)?;
        check(identity==Identity::from(&descriptor_stat(source)?),"duplicated unit directory differs")?;
        Ok(InstallationRoot{directory,identity,scope:Arc::clone(self)})
    }
}
