// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
//! Held per-invocation service authority. No global payload cgroup is permitted.
//! Entry authority comes from protected installation, kernel namespace/proc
//! evidence and authenticated PID1 effective unit policy. Original public owners
//! and the absolute invocation cutoff bind before placement; native qualification
//! and default selector enablement remain pending. No path/env/PPID1-only mint.
use super::*;
use std::sync::Arc;

// Only this module may construct either type. There is no public/unsafe raw-FD
// constructor and no environment/request-path conversion into a trusted token.
use super::super::super::super::{manager,entry,invocation,delegation};
use std::rc::Rc;
struct CheckedEntryOrigin {
    authority:manager::CheckedUnit,
    unit_components:Vec<CString>,
    original:Option<invocation::Metadata>,
    cleanup:Option<Rc<entry::ExitTimer>>,
    delegation:Option<DelegationGrant>,
}
fn checked_entry_origin()->Probe<CheckedEntryOrigin>{
    let until=boottime_ns()?.checked_add(10_000_000_000).ok_or("entry check deadline overflow")?;
    let authority=manager::CheckedUnit::capture(until)?;
    let unit_components=authority.group().split('/').filter(|n|!n.is_empty()).map(|n|CString::new(n).map_err(|_|"manager cgroup NUL")).collect::<std::result::Result<Vec<_>,_>>()?;
    Ok(CheckedEntryOrigin{authority,unit_components,original:None,cleanup:None,delegation:None})
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
    fn require_fixed_policy(&self)->Probe<()>{
        let until=boottime_ns()?.checked_add(10_000_000_000).ok_or("entry validation deadline overflow")?;
        self.authority.validate(until)?;
        check(!self.unit_components.is_empty()&&self.unit_components.len()<=64,"unit path component bound")?;
        let mut total=0usize;
        for component in &self.unit_components{let value=component.to_bytes();check(!value.is_empty()&&value.len()<=255&&value!=b"."&&value!=b".."&&!value.contains(&b'/'),"manager cgroup component refused")?;total=total.checked_add(value.len()+1).filter(|n|*n<=4096).ok_or("manager cgroup byte bound")?;}
        Ok(())
    }
}

impl VerifiedServiceScope {
    pub(in super::super) fn capture_checked_entry() -> Probe<Self> {
        let origin=checked_entry_origin()?;
        Self::open_verified(origin)
    }

    // Origin is minted only by actual private PID1 calls and held protected/
    // kernel authority. Request paths and supplied property snapshots never enter.
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

    pub(crate) fn completion_capabilities(&self)->Probe<(CString,String,[u8;16],OwnedFd,OwnedFd)>{
        let origin=&self.anchor.origin;
        let pidfd=super::super::super::super::pidfd_for(std::process::id() as c_int).map_err(|e|format!("self pidfd: {e:?}"))?;
        let events=fd_from_call(unsafe{fcntl(self.anchor.unit_events.as_raw_fd(),F_DUPFD_CLOEXEC,3 as c_int)})?;
        Ok((origin.authority.name().to_owned(),origin.authority.group().to_owned(),origin.authority.invocation(),pidfd,events))
    }
    // Called only after actual kernel peer/packet/requester validation, before
    // RuntimeMax extension, private listener creation or any payload provision.
    pub(in super::super::super::super) fn bind_delegation(&mut self,uid:u32,metadata:&invocation::Metadata)->Probe<()>{
        check(metadata.separate_service,"explicit separate-service acknowledgment missing")?;
        let anchor=Arc::get_mut(&mut self.anchor).ok_or("service authority shared before delegation binding")?;
        check(anchor.origin.delegation.is_none()&&anchor.origin.original.is_none(),"delegation admission reused")?;
        anchor.origin.delegation=Some(DelegationGrant::capture(uid));
        anchor.origin.delegation.as_ref().ok_or("delegation owner missing")?.revalidate(uid)
    }
    pub(in super::super) fn require_delegation(&self,uid:u32)->Probe<()>{
        self.anchor.origin.delegation.as_ref().ok_or("service has no administrator delegation grant")?.revalidate(uid)
    }
    pub(in super::super::super::super) fn bind_original(mut self,original:&invocation::VerifiedInvocation,cleanup:Rc<entry::ExitTimer>)->std::result::Result<Self,(Self,String)>{
        let result=(||->Probe<()>{
            original.check_live().map_err(|e|format!("{e:?}"))?;original.check_time().map_err(|e|format!("{e:?}"))?;
            let until=original.metadata().deadline_ns.checked_add(entry::GRACE).ok_or("cleanup cutoff overflow")?;cleanup.confirmed_until(until)?;
            let anchor=Arc::get_mut(&mut self.anchor).ok_or("service authority was shared before run binding")?;
            check(anchor.origin.original.is_none()&&anchor.origin.cleanup.is_none(),"service original binding reused")?;
            let grant=anchor.origin.delegation.as_ref().ok_or("delegation must precede runtime extension")?;
            check(original.metadata().separate_service,"original invocation lacks separate-service acknowledgment")?;grant.revalidate(grant.uid)?;
            anchor.origin.authority.bind_runtime(until)?;
            cleanup.confirmed_until(until)?;
            anchor.origin.original=Some(original.metadata().clone());anchor.origin.cleanup=Some(cleanup);
            anchor.revalidate()?;Ok(())
        })();match result{Ok(())=>Ok(self),Err(error)=>Err((self,error))}
    }
    pub(in super::super) fn authorize_original(&self,metadata:&invocation::Metadata,uid:u32)->Probe<()>{
        self.require_delegation(uid)?;check(metadata.separate_service,"separate-service acknowledgment missing at native admission")?;
        let origin=&self.anchor.origin;check(origin.original.as_ref()==Some(metadata),"original invocation differs from established root entry")?;
        origin.cleanup.as_ref().ok_or("entry has no sticky cleanup authority")?.confirmed_until(metadata.deadline_ns.checked_add(entry::GRACE).ok_or("cleanup cutoff overflow")?)
    }
    pub(in super::super) fn retain_anchor(&self) -> Arc<TrustedUnitAnchor> {Arc::clone(&self.anchor)}
    pub(crate) fn revalidate(&self) -> Probe<()> {self.anchor.revalidate()}
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
        let grant=self.origin.delegation.as_ref().ok_or("delegation missing before payload provision")?;grant.revalidate(grant.uid)?;
        self.revalidate()?;
        let source=self.unit_directory()?;
        let directory=fd_from_call(unsafe{fcntl(source.as_raw_fd(),F_DUPFD_CLOEXEC,3 as c_int)})?;
        let identity=require_root_mode(&descriptor_stat(&directory)?,S_IFDIR)?;
        check(identity==Identity::from(&descriptor_stat(source)?),"duplicated unit directory differs")?;
        Ok(InstallationRoot{directory,identity,scope:Arc::clone(self)})
    }
}


// Kernel/protected-file authority used BEFORE contacting PID1. The initial-map
// rule is combined with nsfs owner/type relations and the authenticated manager;
// NS_GET_PARENT/EPERM or visible-proc1 equality alone is never an origin proof.
const PROC_MAGIC:i64=0x9fa0;
const NSFS_MAGIC:i64=0x6e736673;
const NS_GET_USERNS:u64=0xb701;
const NS_GET_PARENT:u64=0xb702;
const NS_GET_NSTYPE:u64=0xb703;
const NS_GET_OWNER_UID:u64=0xb704;
unsafe extern "C"{fn ioctl(fd:c_int,operation:u64,...)->c_int;fn readlinkat(fd:c_int,name:*const c_char,buffer:*mut u8,bytes:usize)->isize;}
struct ProtectedEntry{fd:OwnedFd,identity:Identity,parent:Option<usize>,name:CString,kind:u16}
// Admission snapshot only: no promise of immediate active-run revocation.
// Even a denied/failed capture remains owned by the original unit anchor.
struct DelegationGrant {
 uid:u32,held:Vec<ProtectedEntry>,file:Option<usize>,bytes:Vec<u8>,error:Option<String>,
}
impl DelegationGrant {
 fn capture(uid:u32)->Self{
  let mut value=Self{uid,held:Vec::new(),file:None,bytes:Vec::new(),error:None};
  let result=(||->Probe<()>{
   check(uid!=0&&uid!=u32::MAX,"delegation requester UID refused")?;
   let root=directory(AT_FDCWD,c"/")?;let identity=require_root_mode(&descriptor_stat(&root)?,S_IFDIR)?;
   value.held.push(ProtectedEntry{fd:root,identity,parent:None,name:CString::new("/").expect("literal"),kind:S_IFDIR});
   let mut parent=0;
   for (name,kind) in [(c"etc",S_IFDIR),(c"process-tree-guard",S_IFDIR),(c"delegation.policy",S_IFREG)]{
    let fd=if kind==S_IFDIR{directory(value.held[parent].fd.as_raw_fd(),name)?}else{open_relative(value.held[parent].fd.as_raw_fd(),name,O_NONBLOCK,S_IFREG)?};
    let identity=require_root_mode(&descriptor_stat(&fd)?,kind)?;let next=value.held.len();
    value.held.push(ProtectedEntry{fd,identity,parent:Some(parent),name:name.to_owned(),kind});parent=next;
   }
   value.file=Some(parent);value.check_provenance()?;
   value.bytes=value.read_stable()?;
   value.check_grant(uid)?;value.revalidate(uid)
  })();if let Err(error)=result{value.error=Some(error);}value
 }
 fn check_provenance(&self)->Probe<()>{
  check(self.held.len()==4&&self.file==Some(3),"delegation policy authority incomplete")?;
  for item in &self.held{
   check(require_root_mode(&descriptor_stat(&item.fd)?,item.kind)?==item.identity,"delegation policy held type/mode/mount/identity changed")?;
   if let Some(parent)=item.parent{check(require_name_matches(self.held[parent].fd.as_raw_fd(),&item.name,&item.fd)?==item.identity,"delegation policy name binding changed")?;}
  }Ok(())
 }
 fn read_stable(&self)->Probe<Vec<u8>>{
  let fd=&self.held[self.file.ok_or("delegation policy file missing")?].fd;
  let before=descriptor_stat(fd)?;check(before.size<=delegation::MAX_BYTES as u64,"delegation policy exceeds byte bound")?;
  let bytes=bounded_fd_bytes(fd,delegation::MAX_BYTES)?;let again=bounded_fd_bytes(fd,delegation::MAX_BYTES)?;
  let after=descriptor_stat(fd)?;
  check(bytes==again&&before.size==after.size&&after.size==bytes.len() as u64
   &&before.change.seconds==after.change.seconds&&before.change.nanoseconds==after.change.nanoseconds
   &&before.modified.seconds==after.modified.seconds&&before.modified.nanoseconds==after.modified.nanoseconds,
   "delegation policy changed during capture")?;self.check_provenance()?;Ok(bytes)
 }
 fn check_grant(&self,uid:u32)->Probe<()>{
  check(uid==self.uid,"delegation UID differs from authenticated admission")?;
  let policy=delegation::parse(&self.bytes).map_err(|error|format!("delegation policy grammar: {error:?}"))?;
  check(policy.permits(uid),"no administrator separate-service grant for authenticated UID")
 }
 fn revalidate(&self,uid:u32)->Probe<()>{
  if let Some(error)=&self.error{return Err(error.clone());}
  self.check_provenance()?;check(self.read_stable()?==self.bytes,"delegation policy differs from admission snapshot")?;self.check_grant(uid)
 }
}
pub(crate) struct EntryKernelAuthority{
 protected:Vec<ProtectedEntry>,proc:OwnedFd,proc_identity:Identity,own_proc:OwnedFd,one_proc:OwnedFd,maps:[OwnedFd;2],
 namespaces:Vec<OwnedFd>,namespace_identities:Vec<Identity>,manager:OwnedFd,
}
fn require_fs(fd:&OwnedFd,kind:i64)->Probe<()>{let mut value=Statfs::default();native(os_result(unsafe{fd_statfs(fd.as_raw_fd(),&mut value)} as isize))?;check(value.kind==kind,"entry file is on an unexpected filesystem")}
fn bounded_fd_bytes(fd:&OwnedFd,cap:usize)->Probe<Vec<u8>>{
 use std::io::{Read,Seek,SeekFrom};let duplicate=fd_from_call(unsafe{fcntl(fd.as_raw_fd(),F_DUPFD_CLOEXEC,3 as c_int)})?;
 let mut file=fs::File::from(duplicate);io_result(file.seek(SeekFrom::Start(0)))?;let mut raw=Vec::new();io_result(file.take(cap as u64+1).read_to_end(&mut raw))?;check(raw.len()<=cap,"entry file exceeds its bound")?;Ok(raw)
}
fn open_protected(path:&str,held:&mut Vec<ProtectedEntry>)->Probe<usize>{
 check(path.starts_with('/')&&path.len()<=4096,"invalid installation path")?;
 let root=directory(AT_FDCWD,c"/")?;let identity=require_root_mode(&descriptor_stat(&root)?,S_IFDIR)?;
 let mut parent=held.len();held.push(ProtectedEntry{fd:root,identity,parent:None,name:CString::new("/").expect("literal"),kind:S_IFDIR});
 let names:Vec<_>=path.split('/').filter(|n|!n.is_empty()).collect();check(!names.is_empty()&&names.len()<=64,"installation path depth")?;
 for (index,name) in names.iter().enumerate(){check(*name!="."&&*name!=".."&&name.len()<=255,"installation component refused")?;let name=CString::new(*name).map_err(|_|"installation NUL")?;
  let last=index+1==names.len();let kind=if last{S_IFREG}else{S_IFDIR};
  let fd=if last{fd_from_call(unsafe{fd_openat(held[parent].fd.as_raw_fd(),name.as_ptr(),O_CLOEXEC|O_NOFOLLOW)})?}else{directory(held[parent].fd.as_raw_fd(),&name)?};
  let identity=require_root_mode(&descriptor_stat(&fd)?,kind)?;let next=held.len();held.push(ProtectedEntry{fd,identity,parent:Some(parent),name,kind});parent=next;
 }Ok(parent)
}
fn require_proc_self(proc:&OwnedFd)->Probe<()>{
 require_fs(proc,PROC_MAGIC)?;let identity=Identity::from(&descriptor_stat(proc)?);let link=metadata(proc.as_raw_fd(),c"self",AT_SYMLINK_NOFOLLOW)?;
 check(link.mode&S_IFMT==0o120000&&Identity::from(&link).same_mount(identity),"proc self is not same-mount kernel magic link")?;
 let mut bytes=[0u8;32];let count=native(os_result(unsafe{readlinkat(proc.as_raw_fd(),c"self".as_ptr(),bytes.as_mut_ptr(),bytes.len())}))?;
 check(count<bytes.len()&&&bytes[..count]==std::process::id().to_string().as_bytes(),"procfs self PID space differs from actual caller")
}
// Procfs task directories are kernel-generated and may belong to the ordinary
// caller or change ownership with dumpability. Their authority is the genuine
// held proc mount/name/type binding, not the installation/cgroup UID0 rule.
fn proc_directory(parent:&OwnedFd,name:&CStr)->Probe<OwnedFd>{
 require_fs(parent,PROC_MAGIC)?;let parent_identity=Identity::from(&descriptor_stat(parent)?);
 let child=open_relative(parent.as_raw_fd(),name,O_PATH|O_DIRECTORY,S_IFDIR)?;
 require_fs(&child,PROC_MAGIC)?;let raw=descriptor_stat(&child)?;
 check(raw.mode&S_IFMT==S_IFDIR&&Identity::from(&raw).same_mount(parent_identity),"proc child directory mount/type differs")?;
 require_name_matches(parent.as_raw_fd(),name,&child)?;Ok(child)
}
fn proc_process(proc:&OwnedFd,pid:u32)->Probe<OwnedFd>{let name=CString::new(pid.to_string()).expect("decimalPID");let fd=proc_directory(proc,&name)?;require_fs(&fd,PROC_MAGIC)?;let root=Identity::from(&descriptor_stat(proc)?);let raw=descriptor_stat(&fd)?;check(raw.mode&S_IFMT==S_IFDIR&&Identity::from(&raw).same_mount(root),"proc task directory mount/type differs")?;Ok(fd)}
fn proc_leaf(process:&OwnedFd,name:&CStr)->Probe<OwnedFd>{let fd=fd_from_call(unsafe{fd_openat(process.as_raw_fd(),name.as_ptr(),O_CLOEXEC|O_NOFOLLOW)})?;require_fs(&fd,PROC_MAGIC)?;let raw=descriptor_stat(&fd)?;check(raw.mode&S_IFMT==S_IFREG&&Identity::from(&raw).same_mount(Identity::from(&descriptor_stat(process)?)),"proc leaf mount/type differs")?;Ok(fd)}
fn namespace(proc_process:&OwnedFd,name:&CStr,kind:c_int)->Probe<OwnedFd>{
 let ns=proc_directory(proc_process,c"ns")?;require_fs(&ns,PROC_MAGIC)?;let mount=Identity::from(&descriptor_stat(proc_process)?);check(Identity::from(&descriptor_stat(&ns)?).same_mount(mount),"proc namespace directory mount differs")?;
 let link=metadata(ns.as_raw_fd(),name,AT_SYMLINK_NOFOLLOW)?;check(link.mode&S_IFMT==0o120000&&Identity::from(&link).same_mount(mount),"namespace leaf is not same-mount proc magic link")?;
 let fd=fd_from_call(unsafe{fd_openat(ns.as_raw_fd(),name.as_ptr(),O_CLOEXEC)})?;require_fs(&fd,NSFS_MAGIC)?;
 let actual=unsafe{ioctl(fd.as_raw_fd(),NS_GET_NSTYPE)};native(os_result(actual as isize))?;check(actual==kind,"namespace type differs")?;Ok(fd)
}
fn checked_maps(process:&OwnedFd)->Probe<[OwnedFd;2]>{
 let mut held=Vec::new();for name in [c"uid_map",c"gid_map"]{let fd=proc_leaf(process,name)?;let raw=bounded_fd_bytes(&fd,4096)?;require_full_map(&raw)?;held.push(fd);}
 held.try_into().map_err(|_|"mapping descriptor count".into())
}
fn require_full_map(raw:&[u8])->Probe<()>{let text=std::str::from_utf8(raw).map_err(|_|"namespace mapping UTF8")?;let numbers:Vec<_>=text.split_whitespace().map(|n|n.parse::<u64>().map_err(|_|"namespace mapping number")).collect::<std::result::Result<_,_>>()?;check(numbers==[0,0,4_294_967_295],"entry requires full initial UID/GID identity mapping authority")}
pub(in super::super::super) fn require_initial_mappings()->Probe<()>{
 let proc=directory(AT_FDCWD,c"/proc")?;require_fs(&proc,PROC_MAGIC)?;require_root_mode(&descriptor_stat(&proc)?,S_IFDIR)?;require_proc_self(&proc)?;let process=proc_process(&proc,std::process::id())?;let _held=checked_maps(&process)?;Ok(())
}
impl EntryKernelAuthority{
 pub(in super::super::super) fn capture()->Probe<Self>{
  let proc=directory(AT_FDCWD,c"/proc")?;require_fs(&proc,PROC_MAGIC)?;let proc_identity=require_root_mode(&descriptor_stat(&proc)?,S_IFDIR)?;require_proc_self(&proc)?;
  let own=proc_process(&proc,std::process::id())?;let one=proc_process(&proc,1)?;let maps=checked_maps(&own)?;
  let status_fd=proc_leaf(&own,c"status")?;let status=bounded_fd_bytes(&status_fd,65536)?;let status=std::str::from_utf8(&status).map_err(|_|"entry status UTF8")?;
  check(status_ids(status,"Uid:")?==[0;4]&&status_ids(status,"Gid:")?==[0;4]&&status_field(status,"TracerPid:")?=="0"
    &&unsafe{getuid()}==0&&unsafe{geteuid()}==0&&unsafe{getgid()}==0&&unsafe{getegid()}==0,"entry requires untraced all-root credentials")?;
  let mut protected=Vec::new();let helper=open_protected(super::super::super::super::manager::HELPER,&mut protected)?;
  let socket=open_protected(super::super::super::super::manager::SOCKET_FILE,&mut protected)?;
  let template=open_protected(super::super::super::super::manager::TEMPLATE_FILE,&mut protected)?;
  check(bounded_fd_bytes(&protected[socket].fd,65536)?==include_bytes!("deployment/process-tree-guard.socket"),"installed socket source differs")?;
  check(bounded_fd_bytes(&protected[template].fd,65536)?==include_bytes!("deployment/process-tree-guard@.service"),"installed template source differs")?;

  let exe_link=metadata(own.as_raw_fd(),c"exe",AT_SYMLINK_NOFOLLOW)?;check(exe_link.mode&S_IFMT==0o120000&&Identity::from(&exe_link).same_mount(proc_identity),"self exe link is not original proc mount")?;
  let exe=fd_from_call(unsafe{fd_openat(own.as_raw_fd(),c"exe".as_ptr(),O_CLOEXEC)})?;
  check(require_root_mode(&descriptor_stat(&exe)?,S_IFREG)?==protected[helper].identity,"running executable is not protected installed helper")?;
  let manager=super::super::super::super::pidfd_for(1).map_err(|e|format!("PID1 authority: {e:?}"))?;
  super::super::super::super::validate_pidfd(&manager,1,&mut NativeReceipt::default()).map_err(|e|format!("PID1 authority: {e:?}"))?;
  let kinds=[(c"user",0x10000000),(c"pid",0x20000000),(c"mnt",0x00020000),(c"time",0x00000080),(c"cgroup",0x02000000)];
  let mut namespaces=Vec::new();let mut namespace_identities=Vec::new();let mut initial_user=None;
  for (name,kind) in kinds{
   let current=namespace(&own,name,kind)?;let host=namespace(&one,name,kind)?;let identity=Identity::from(&descriptor_stat(&current)?);
   check(identity==Identity::from(&descriptor_stat(&host)?),"service differs from authenticated manager namespace")?;
   if kind==0x10000000{
    let mut owner=u32::MAX;native(os_result(unsafe{ioctl(current.as_raw_fd(),NS_GET_OWNER_UID,&mut owner as *mut u32)} as isize))?;check(owner==0,"user namespace owner is not root")?;
    let parent=unsafe{ioctl(current.as_raw_fd(),NS_GET_PARENT)};
    if parent>=0{drop(unsafe{OwnedFd::from_raw_fd(parent)});return Err("entry user namespace has an accessible parent".into());}
    check(io::Error::last_os_error().raw_os_error()==Some(1),"unexpected namespace parent result")?;initial_user=Some(identity);
   }else{
    let owner=fd_from_call(unsafe{ioctl(current.as_raw_fd(),NS_GET_USERNS)})?;require_fs(&owner,NSFS_MAGIC)?;
    check(Some(Identity::from(&descriptor_stat(&owner)?))==initial_user,"namespace is owned outside checked initial user authority")?;
    namespace_identities.push(Identity::from(&descriptor_stat(&owner)?));namespaces.push(owner);
   }
   for fd in [current,host]{namespace_identities.push(Identity::from(&descriptor_stat(&fd)?));namespaces.push(fd);}
  }
  namespace_identities.push(Identity::from(&descriptor_stat(&exe)?));namespaces.push(exe);
  let result=Self{protected,proc,proc_identity,own_proc:own,one_proc:one,maps,namespaces,namespace_identities,manager};result.revalidate()?;Ok(result)
 }
 pub(crate) fn bind_runtime_overrides(&mut self,paths:&[String],unit:&CStr)->Probe<()>{
  check(paths.len()<=1,"manager created unexpected additional run overrides")?;
  let unit=unit.to_str().map_err(|_|"unit override UTF8")?;check(!unit.contains('/')&&unit.len()<=255,"unit override component refused")?;
  let prefix=format!("/run/systemd/system.control/{unit}.d/");
  for path in paths{let name=path.strip_prefix(&prefix).ok_or("runtime override is outside exact own-unit control directory")?;
   check(!name.is_empty()&&name.len()<=255&&!name.contains('/')&&name.ends_with(".conf")&&name.bytes().all(|b|b.is_ascii_alphanumeric()||b"._-".contains(&b)),"runtime override filename refused")?;
   let file=open_protected(path,&mut self.protected)?;let _raw=bounded_fd_bytes(&self.protected[file].fd,65536)?;
  }Ok(())
 }
 pub(crate) fn revalidate(&self)->Probe<()>{
  require_fs(&self.proc,PROC_MAGIC)?;require_proc_self(&self.proc)?;
  let own_name=CString::new(std::process::id().to_string()).expect("decimalPID");require_name_matches(self.proc.as_raw_fd(),&own_name,&self.own_proc)?;require_name_matches(self.proc.as_raw_fd(),c"1",&self.one_proc)?;
  for (name,fd) in [c"uid_map",c"gid_map"].into_iter().zip(&self.maps){require_fs(fd,PROC_MAGIC)?;let raw=descriptor_stat(fd)?;check(raw.mode&S_IFMT==S_IFREG&&Identity::from(&raw).same_mount(self.proc_identity),"held map file mount/type changed")?;require_name_matches(self.own_proc.as_raw_fd(),name,fd)?;require_full_map(&bounded_fd_bytes(fd,4096)?)?;}
check(require_root_mode(&descriptor_stat(&self.proc)?,S_IFDIR)?==self.proc_identity,"trusted proc identity changed")?;
  super::super::super::super::validate_pidfd(&self.manager,1,&mut NativeReceipt::default()).map_err(|e|format!("PID1 lost: {e:?}"))?;
  for item in &self.protected{check(require_root_mode(&descriptor_stat(&item.fd)?,item.kind)?==item.identity,"protected entry identity changed")?;if let Some(parent)=item.parent{require_name_matches(self.protected[parent].fd.as_raw_fd(),&item.name,&item.fd)?;}}
  for (fd,identity) in self.namespaces.iter().zip(&self.namespace_identities){check(Identity::from(&descriptor_stat(fd)?)==*identity,"held entry namespace/image identity changed")?;}
  Ok(())
 }
}


pub(in super::super::super) fn client_installation()->Probe<Vec<OwnedFd>>{
 require_initial_mappings()?;let mut held=Vec::new();
 let _helper=open_protected(manager::HELPER,&mut held)?;
 for (path,expected) in [(manager::SOCKET_FILE,include_bytes!("deployment/process-tree-guard.socket").as_slice()),(manager::TEMPLATE_FILE,include_bytes!("deployment/process-tree-guard@.service").as_slice())]{let file=open_protected(path,&mut held)?;check(bounded_fd_bytes(&held[file].fd,65536)?==expected,"client installation source differs")?;}
 let root=directory(AT_FDCWD,c"/")?;require_root_mode(&descriptor_stat(&root)?,S_IFDIR)?;let run=directory(root.as_raw_fd(),c"run")?;require_root_mode(&descriptor_stat(&run)?,S_IFDIR)?;let parent=directory(run.as_raw_fd(),c"process-tree-guard")?;require_root_mode(&descriptor_stat(&parent)?,S_IFDIR)?;
 let socket=fd_from_call(unsafe{fd_openat(parent.as_raw_fd(),c"activate.sock".as_ptr(),O_PATH|O_CLOEXEC|O_NOFOLLOW)})?;let raw=descriptor_stat(&socket)?;
 check(raw.uid==0&&raw.gid==0&&raw.mode&S_IFMT==0o140000,"activation node is not protected root socket")?;
 let mut fds:Vec<_>=held.into_iter().map(|item|item.fd).collect();fds.extend([root,run,parent,socket]);Ok(fds)
}
// Client-side completion authority is captured while this exact service unit is
// live/populated. No later pathname reopen can stand in for the original domain.
struct CompletionNode{fd:OwnedFd,identity:Identity,name:Option<CString>,cgroup:bool}
#[derive(Clone)]
struct EventRead{bytes:Vec<u8>,seek_errno:Option<c_int>,read_errno:Option<c_int>}
impl EventRead{
 fn populated(&self)->Probe<bool>{
  check(self.seek_errno.is_none()&&self.read_errno.is_none()&&!self.bytes.is_empty()&&self.bytes.len()<=4096,"completion event read not complete")?;
  let text=std::str::from_utf8(&self.bytes).map_err(|_|"unit events UTF8")?;let mut populated=None;
  for line in text.lines(){let mut words=line.split_whitespace();let key=words.next().ok_or("unit event key")?;let value=words.next().ok_or("unit event value")?;check(words.next().is_none(),"unit event shape")?;if key=="populated"{check(populated.is_none(),"duplicate populated event")?;populated=Some(match value{"0"=>false,"1"=>true,_=>return Err("unit populated value".into())});}}
  populated.ok_or_else(||"unit events omitted populated".into())
 }
}
fn read_completion_events(fd:&OwnedFd)->EventRead{
 use std::io::{Seek,SeekFrom};
 let duplicate=unsafe{fcntl(fd.as_raw_fd(),F_DUPFD_CLOEXEC,3 as c_int)};
 if duplicate<0{return EventRead{bytes:Vec::new(),seek_errno:io::Error::last_os_error().raw_os_error(),read_errno:None};}
 let mut file=fs::File::from(unsafe{OwnedFd::from_raw_fd(duplicate)});
 if let Err(error)=file.seek(SeekFrom::Start(0)){return EventRead{bytes:Vec::new(),seek_errno:error.raw_os_error(),read_errno:None};}
 let mut raw=Vec::with_capacity(4097);let mut chunk=[0u8;4097];
 loop{let count=unsafe{fd_read(file.as_raw_fd(),chunk.as_mut_ptr().cast(),4097-raw.len())};
  if count<0{return EventRead{bytes:raw,seek_errno:None,read_errno:io::Error::last_os_error().raw_os_error()};}
  if count==0{return EventRead{bytes:raw,seek_errno:None,read_errno:None};}
  raw.extend_from_slice(&chunk[..count as usize]);if raw.len()>=4097{return EventRead{bytes:raw,seek_errno:None,read_errno:None};}
 }
}
struct RetirementLookup{result:c_int,errno:Option<c_int>,observed:Option<Identity>}
#[must_use="original unit completion capabilities must remain held"]
pub(crate) struct ClientCompletion{nodes:Vec<CompletionNode>,events:OwnedFd,event_identity:Identity,group:String,baseline:EventRead,last:Option<EventRead>,first_failure:Option<EventRead>,last_lookup:Option<RetirementLookup>,retired:Option<RetiredUnit>}
// Private fields: only the exact ENODEV/held-parent ENOENT path below mints this.
// It proves retirement/emptiness, not child reaping or CSS-reference release.
pub(crate) struct RetiredUnit{group:String,unit:Identity,parent:Identity,name:CString,event:Identity,event_errno:c_int,lookup_result:c_int,lookup_errno:c_int,observed_ns:u64}
impl RetiredUnit{pub(crate) fn matches_group(&self,group:&str)->bool{self.group==group}}
impl ClientCompletion{
 fn revalidate_ancestry(&self,bind_unit:bool)->Probe<()>{
  let actual=directory(AT_FDCWD,c"/")?;check(require_root_mode(&descriptor_stat(&actual)?,S_IFDIR)?==self.nodes[0].identity,"completion root view changed")?;
  let last=self.nodes.len()-1;
  for(index,node)in self.nodes.iter().enumerate(){check(require_root_mode(&descriptor_stat(&node.fd)?,S_IFDIR)?==node.identity,"held completion directory changed")?;if node.cgroup{require_cgroup2(&node.fd)?;}if index>0&&(index!=last||bind_unit){require_name_matches(self.nodes[index-1].fd.as_raw_fd(),node.name.as_ref().ok_or("completion component name missing")?,&node.fd)?;}}
  require_cgroup2(&self.events)?;check(require_root_mode(&descriptor_stat(&self.events)?,S_IFREG)?==self.event_identity,"held completion event identity changed")?;Ok(())
 }
 pub(crate) fn population_after_terminal(&mut self,service:&OwnedFd,pid:u32)->Probe<bool>{
  // No retirement inference is permitted until this ORIGINAL service object is
  // terminal. No pidfd_open or numeric-PID replacement occurs here.
  let mut event=super::super::super::super::PollFd{fd:service.as_raw_fd(),events:1,revents:0};
  let count=unsafe{super::super::super::super::poll(&mut event,1,0)};
  check(count==1&&event.revents&(1|0x10)!=0&&event.revents&!(1|0x10)==0&&pid>1,"original service pidfd is not terminal")?;
  self.revalidate_ancestry(false)?;
  let raw=read_completion_events(&self.events);let decision=raw.populated();self.last=Some(raw.clone());
  match decision{
   Ok(value)=>{self.revalidate_ancestry(true)?;Ok(value)},
   Err(error)=>{
    if self.first_failure.is_none(){self.first_failure=Some(raw.clone());}
    // Only the raw read ENODEV of the original, previously-live core file
    // enables retirement lookup. Seek/parse/EOF/EACCES/ESTALE never do so.
    check(raw.seek_errno.is_none()&&raw.read_errno==Some(19),&error)?;
    let unit=self.nodes.last().ok_or("original completion unit missing")?;let parent=&self.nodes[self.nodes.len()-2];let name=unit.name.as_ref().ok_or("original unit basename missing")?;
    let mut observed=Statx::default();let result=unsafe{fd_statx(parent.fd.as_raw_fd(),name.as_ptr(),AT_SYMLINK_NOFOLLOW,STATX_BASIC_STATS|STATX_MNT_ID,&mut observed)};
    let errno=if result<0{io::Error::last_os_error().raw_os_error()}else{None};
    self.last_lookup=Some(RetirementLookup{result,errno,observed:if result==0{Some(Identity::from(&observed))}else{None}});
    // A replacement/different inode is NOT the original and cannot settle it.
    check(result<0&&errno==Some(2),"original unit name is not strictly absent under held parent")?;
    self.revalidate_ancestry(false)?;
    self.retired=Some(RetiredUnit{group:self.group.clone(),unit:unit.identity,parent:parent.identity,name:name.clone(),event:self.event_identity,event_errno:19,lookup_result:result,lookup_errno:2,observed_ns:boottime_ns()?});Ok(false)
   }
  }
 }
 pub(crate) fn retired(&self)->Option<&RetiredUnit>{self.retired.as_ref()}
}
pub(in super::super::super) fn client_events(fd:&OwnedFd,group:&str)->Probe<ClientCompletion>{
 check(group.starts_with('/')&&group.len()<=4096,"completion cgroup path bound")?;
 let mut nodes=Vec::new();let root=directory(AT_FDCWD,c"/")?;let identity=require_root_mode(&descriptor_stat(&root)?,S_IFDIR)?;nodes.push(CompletionNode{fd:root,identity,name:None,cgroup:false});
 for name in [c"sys",c"fs",c"cgroup"]{let parent=nodes.last().expect("root held");let child=directory(parent.fd.as_raw_fd(),name)?;let identity=require_root_mode(&descriptor_stat(&child)?,S_IFDIR)?;let cgroup=name==c"cgroup";if cgroup{require_cgroup2(&child)?;}nodes.push(CompletionNode{fd:child,identity,name:Some(name.to_owned()),cgroup});}
 let mount=nodes.last().expect("hierarchy held").identity;let mut count=0;
 for name in group.split('/').filter(|n|!n.is_empty()){
  count+=1;check(count<=64&&name.len()<=255&&name!="."&&name!="..","completion cgroup component")?;
  let name=CString::new(name).map_err(|_|"completion cgroup NUL")?;let parent=nodes.last().expect("hierarchy retained");let child=directory(parent.fd.as_raw_fd(),&name)?;require_cgroup2(&child)?;let identity=require_root_mode(&descriptor_stat(&child)?,S_IFDIR)?;check(identity.same_mount(mount),"completion unit changes mount")?;nodes.push(CompletionNode{fd:child,identity,name:Some(name),cgroup:true});
 }
 check(count>0,"completion unit cannot be hierarchy")?;require_cgroup2(fd)?;let event_identity=require_root_mode(&descriptor_stat(fd)?,S_IFREG)?;check(event_identity.same_mount(mount),"completion events mount differs")?;
 require_name_matches(nodes.last().expect("unit held").fd.as_raw_fd(),c"cgroup.events",fd)?;super::super::super::super::close_on_exec(fd.as_raw_fd()).map_err(|e|format!("{e:?}"))?;
 let flags=super::super::super::super::io_flags(fd.as_raw_fd(),0).map_err(|e|format!("{e:?}"))?;check(flags&3==0,"completion event capability is not read-only")?;
 let events=fd_from_call(unsafe{fcntl(fd.as_raw_fd(),F_DUPFD_CLOEXEC,3 as c_int)})?;let baseline=read_completion_events(&events);check(baseline.populated()?,"original service was not live/populated at completion adoption")?;
 let result=ClientCompletion{nodes,events,event_identity,group:group.to_owned(),baseline,last:None,first_failure:None,last_lookup:None,retired:None};result.revalidate_ancestry(true)?;Ok(result)
}


pub(in super::super::super) fn verify_runtime_parent(fds:&[OwnedFd])->Probe<()>{
 check(fds.len()>=4,"runtime parent custody missing")?;let n=fds.len();let root=&fds[n-4];let run=&fds[n-3];let parent=&fds[n-2];
 let actual=directory(AT_FDCWD,c"/")?;check(require_root_mode(&descriptor_stat(root)?,S_IFDIR)?==require_root_mode(&descriptor_stat(&actual)?,S_IFDIR)?,"held runtime root differs from actual root")?;
 require_root_mode(&descriptor_stat(run)?,S_IFDIR)?;require_root_mode(&descriptor_stat(parent)?,S_IFDIR)?;
 require_name_matches(root.as_raw_fd(),c"run",run)?;require_name_matches(run.as_raw_fd(),c"process-tree-guard",parent)?;Ok(())
}

// Protected fixed system-bus pathname is the trust root for its owner replies.
// The daemon may legitimately run as a nonroot message-bus account.
pub(crate) struct PublicBusEndpoint{root:OwnedFd,run:OwnedFd,parent:OwnedFd,socket:OwnedFd,identities:[Identity;4],peer:Option<OwnedFd>,peer_credentials:Option<Credentials>}
impl PublicBusEndpoint{
 pub(crate) fn capture()->Probe<Self>{
  require_initial_mappings()?;let root=directory(AT_FDCWD,c"/")?;let run=directory(root.as_raw_fd(),c"run")?;let parent=directory(run.as_raw_fd(),c"dbus")?;
  let socket=fd_from_call(unsafe{fd_openat(parent.as_raw_fd(),c"system_bus_socket".as_ptr(),O_PATH|O_CLOEXEC|O_NOFOLLOW)})?;
  let mut identities=[Identity::from(&descriptor_stat(&root)?);4];
  for(index,fd)in [&root,&run,&parent].into_iter().enumerate(){identities[index]=require_root_mode(&descriptor_stat(fd)?,S_IFDIR)?;}
  let raw=descriptor_stat(&socket)?;check(raw.uid==0&&raw.gid==0&&raw.mode&S_IFMT==0o140000,"system bus socket is not a root-controlled socket")?;identities[3]=Identity::from(&raw);
  let value=Self{root,run,parent,socket,identities,peer:None,peer_credentials:None};value.revalidate()?;Ok(value)
 }
 pub(crate) fn authenticate(&mut self,fd:RawFd)->Probe<()>{
  self.revalidate()?;let peer=super::super::super::super::kernel_peer_credentials(fd).map_err(|e|format!("system bus peer: {e:?}"))?;
  check(peer.pid>1||(peer.pid==1&&peer.uid==0&&peer.gid==0),"public bus peer identity refused")?;
  let identity=super::super::super::super::pidfd_for(peer.pid).map_err(|e|format!("system bus daemon pidfd: {e:?}"))?;
  self.peer=Some(identity);self.peer_credentials=Some(peer);
  super::super::super::super::validate_pidfd(self.peer.as_ref().expect("daemon owned"),peer.pid,&mut NativeReceipt::default()).map_err(|e|format!("system bus daemon identity: {e:?}"))?;
  check(super::super::super::super::kernel_peer_credentials(fd).map_err(|e|format!("{e:?}"))?==peer,"system bus peer changed")?;self.revalidate()
 }
 pub(crate) fn revalidate(&self)->Probe<()>{
  let actual=directory(AT_FDCWD,c"/")?;check(require_root_mode(&descriptor_stat(&actual)?,S_IFDIR)?==self.identities[0],"public bus root view changed")?;
  for(index,fd)in [&self.root,&self.run,&self.parent].into_iter().enumerate(){check(require_root_mode(&descriptor_stat(fd)?,S_IFDIR)?==self.identities[index],"public bus protected ancestry changed")?;}
  require_name_matches(self.root.as_raw_fd(),c"run",&self.run)?;require_name_matches(self.run.as_raw_fd(),c"dbus",&self.parent)?;require_name_matches(self.parent.as_raw_fd(),c"system_bus_socket",&self.socket)?;
  let raw=descriptor_stat(&self.socket)?;check(raw.uid==0&&raw.gid==0&&raw.mode&S_IFMT==0o140000&&Identity::from(&raw)==self.identities[3],"public bus socket identity changed")?;
  if let(Some(fd),Some(peer))=(&self.peer,self.peer_credentials){super::super::super::super::validate_pidfd(fd,peer.pid,&mut NativeReceipt::default()).map_err(|e|format!("system bus daemon lost: {e:?}"))?;}Ok(())
 }
}

// PID1 owns this per-service directory for the entire instance lifetime and
// removes it after stop, including asynchronous SIGKILL before Rust can unlink.
pub(crate) struct PrivateRuntimeDirectory{installation:Vec<OwnedFd>,directory:OwnedFd,identity:Identity,name:CString}
impl PrivateRuntimeDirectory{
 pub(crate) fn capture(unit:&str)->Probe<Self>{
  check(unit.starts_with("process-tree-guard@")&&unit.ends_with(".service")&&unit.len()<=255&&!unit.contains('/')&&!unit.as_bytes().contains(&0),"runtime unit component refused")?;
  let installation=client_installation()?;let parent=&installation[installation.len()-2];let name=CString::new(unit).map_err(|_|"runtime unit NUL")?;let directory=directory(parent.as_raw_fd(),&name)?;let identity=require_root_mode(&descriptor_stat(&directory)?,S_IFDIR)?;
  let result=Self{installation,directory,identity,name};result.revalidate()?;Ok(result)
 }
 pub(crate) fn path(&self)->Probe<String>{Ok(format!("/run/process-tree-guard/{}/private.sock",self.name.to_str().map_err(|_|"runtime unit UTF8")?))}
 pub(crate) fn revalidate(&self)->Probe<()>{verify_runtime_parent(&self.installation)?;check(require_root_mode(&descriptor_stat(&self.directory)?,S_IFDIR)?==self.identity,"held per-service runtime directory changed")?;let parent=&self.installation[self.installation.len()-2];require_name_matches(parent.as_raw_fd(),&self.name,&self.directory)?;Ok(())}
}

// Genuine calling-thread snapshot. No umask(0)/restore and no main-thread
// substitution when a library caller has a distinct fs_struct.
pub(in super::super::super) fn capture_calling_umask()->Probe<u16>{
 #[cfg(target_arch="aarch64")]const GETTID:c_long=178;
 #[cfg(target_arch="x86_64")]const GETTID:c_long=186;
 let tid=unsafe{super::super::super::super::syscall(GETTID)};check(tid>0&&tid<=c_int::MAX as c_long,"calling thread identity refused")?;
 let proc=directory(AT_FDCWD,c"/proc")?;require_fs(&proc,PROC_MAGIC)?;let mount=require_root_mode(&descriptor_stat(&proc)?,S_IFDIR)?;require_proc_self(&proc)?;
 let link=metadata(proc.as_raw_fd(),c"thread-self",AT_SYMLINK_NOFOLLOW)?;check(link.mode&S_IFMT==0o120000&&Identity::from(&link).same_mount(mount),"thread-self is not same-mount kernel proc link")?;
 let expected=format!("{}/task/{tid}",std::process::id());let mut target=[0u8;96];let count=native(os_result(unsafe{readlinkat(proc.as_raw_fd(),c"thread-self".as_ptr(),target.as_mut_ptr(),target.len())}))?;
 check(count<target.len()&&&target[..count]==expected.as_bytes(),"proc thread-self differs from actual calling thread")?;
 let process=proc_process(&proc,std::process::id())?;let tasks=proc_directory(&process,c"task")?;require_fs(&tasks,PROC_MAGIC)?;check(Identity::from(&descriptor_stat(&tasks)?).same_mount(mount),"proc task directory mount differs")?;
 let name=CString::new(tid.to_string()).expect("decimalTID");let thread=proc_directory(&tasks,&name)?;require_fs(&thread,PROC_MAGIC)?;check(Identity::from(&descriptor_stat(&thread)?).same_mount(mount),"proc calling-thread directory mount differs")?;
 let status=proc_leaf(&thread,c"status")?;let raw=bounded_fd_bytes(&status,65536)?;require_name_matches(thread.as_raw_fd(),c"status",&status)?;
 let text=std::str::from_utf8(&raw).map_err(|_|"calling-thread status UTF8")?;let value=status_field(text,"Umask:")?;
 check(!value.is_empty()&&value.len()<=4&&value.bytes().all(|byte|(b'0'..=b'7').contains(&byte)),"calling-thread umask format refused")?;
 let mask=u16::from_str_radix(value,8).map_err(|_|"calling-thread umask octal")?;check(mask<=0o777,"calling-thread umask exceeds0777")?;Ok(mask)
}
