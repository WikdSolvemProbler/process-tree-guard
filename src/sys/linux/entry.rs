// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
//! Real root one-shot service entry and bounded native channel operations.
use super::*;
use std::ptr::NonNull;
use std::cell::RefCell;
use std::rc::Rc;
use std::os::unix::fs::{MetadataExt,PermissionsExt};
pub(super) const GRACE:u64=5_000_000_000;
const BOOTSTRAP:u64=15_000_000_000;
const POLLOUT:c_short=4;
#[repr(C)]#[derive(Clone,Copy,Debug)]
struct TimerObservation{schema:u32,stage:u32,result:c_int,call_errno:c_int,created:c_int,armed:c_int,
 create_result:c_int,create_errno:c_int,set_result:c_int,set_errno:c_int,get_result:c_int,get_errno:c_int,
 delete_result:c_int,delete_errno:c_int,deadline_ns:u64,observed_ns:u64,remaining_ns:u64,interval_ns:u64,before_ns:u64}
impl Default for TimerObservation{fn default()->Self{Self{schema:0,stage:0,result:c_int::MIN,call_errno:0,created:0,armed:0,create_result:c_int::MIN,create_errno:0,set_result:c_int::MIN,set_errno:0,get_result:c_int::MIN,get_errno:0,delete_result:c_int::MIN,delete_errno:0,deadline_ns:0,observed_ns:0,remaining_ns:0,interval_ns:0,before_ns:0}}}
const _:()={assert!(size_of::<TimerObservation>()==96);assert!(std::mem::offset_of!(TimerObservation,deadline_ns)==56);assert!(std::mem::offset_of!(TimerObservation,before_ns)==88);};
#[link(name="rt")]
unsafe extern "C"{
 fn ptg_entry_timer_abi(word:u32)->u64;
 fn ptg_entry_timer_create(until:u64,owner:*mut *mut c_void,out:*mut TimerObservation);
 fn ptg_entry_cleanup_timer_create(until:u64,owner:*mut *mut c_void,out:*mut TimerObservation);
 fn ptg_entry_timer_observe(owner:*const c_void,out:*mut TimerObservation);
 fn ptg_entry_timer_delete(owner:*mut c_void,out:*mut TimerObservation);
 pub(super) fn _exit(code:c_int)->!;
 fn write(fd:c_int,bytes:*const c_void,count:usize)->isize;
 fn sendto(fd:c_int,bytes:*const c_void,count:usize,flags:c_int,address:*const c_void,length:u32)->isize;
 fn shutdown(fd:c_int,how:c_int)->c_int;
 fn socket(domain:c_int,kind:c_int,protocol:c_int)->c_int;
 fn bind(fd:c_int,address:*const c_void,length:u32)->c_int;
 fn connect(fd:c_int,address:*const c_void,length:u32)->c_int;
 fn listen(fd:c_int,backlog:c_int)->c_int;
 fn accept4(fd:c_int,address:*mut c_void,length:*mut u32,flags:c_int)->c_int;
 fn getsockname(fd:c_int,address:*mut c_void,length:*mut u32)->c_int;
}
type Probe<T>=std::result::Result<T,String>;
#[must_use="kernel self timer remains armed until explicit bootstrap handoff or process exit"]
struct TimerJournal{create:TimerObservation,latest:Option<TimerObservation>,first_failure:Option<TimerObservation>,delete:Option<TimerObservation>,checks:u64}
pub(super) struct ExitTimer{abi:[u64;9],pointer:Option<NonNull<c_void>>,sticky:bool,until:u64,observations:RefCell<TimerJournal>}
struct TimerFailure{owner:Rc<ExitTimer>,error:String}
fn timer_readback(raw:&TimerObservation,until:u64)->bool{
 raw.schema==1&&raw.stage==4&&raw.result==0&&raw.call_errno==0&&raw.created==1&&raw.armed==1
 &&raw.get_result==0&&raw.get_errno==0&&raw.deadline_ns==until&&raw.interval_ns==0&&raw.remaining_ns>0
 &&raw.before_ns>0&&raw.observed_ns>=raw.before_ns&&raw.observed_ns<until
 &&until.checked_sub(raw.remaining_ns).is_some_and(|inferred|inferred>=raw.before_ns&&inferred<=raw.observed_ns)
}
impl ExitTimer{
 fn create(until:u64,sticky:bool)->std::result::Result<Rc<Self>,TimerFailure>{
  let abi:[u64;9]=std::array::from_fn(|i|unsafe{ptg_entry_timer_abi(i as u32)});
  let mut pointer=ptr::null_mut();let mut raw=TimerObservation::default();
  if abi==[1,96,7,9,0,1,64,32,56]{unsafe{if sticky{ptg_entry_cleanup_timer_create(until,&mut pointer,&mut raw)}else{ptg_entry_timer_create(until,&mut pointer,&mut raw)}};}
  // Publish actual custody BEFORE interpreting arm/readback failure.
  let owner=Rc::new(Self{abi,pointer:NonNull::new(pointer),sticky,until,observations:RefCell::new(TimerJournal{create:raw,latest:None,first_failure:None,delete:None,checks:0})});
  if abi!=[1,96,7,9,0,1,64,32,56]||owner.pointer.is_none()||!timer_readback(&raw,until)||raw.create_result!=0||raw.create_errno!=0||raw.set_result!=0||raw.set_errno!=0||raw.delete_result!=c_int::MIN||raw.delete_errno!=0{
   owner.observations.borrow_mut().first_failure=Some(raw);return Err(TimerFailure{owner,error:"native entry timer refused; actual custody retained".into()});}
  Ok(owner)
 }
 pub(super) fn confirmed_until(&self,until:u64)->Probe<()>{
  if self.observations.borrow().first_failure.is_some(){return Err("entry cleanup timer has prior failure".into());}
  if !self.sticky||until!=self.until{return Err("entry cleanup timer identity differs".into());}
  let pointer=self.pointer.ok_or("entry cleanup timer has no native owner")?;let mut raw=TimerObservation::default();
  {let mut journal=self.observations.borrow_mut();journal.checks=journal.checks.checked_add(1).ok_or("entry timer observation counter exhausted")?;}
  unsafe{ptg_entry_timer_observe(pointer.as_ptr(),&mut raw)};self.observations.borrow_mut().latest=Some(raw);
  if !timer_readback(&raw,until)||raw.create_result!=c_int::MIN||raw.create_errno!=0||raw.set_result!=c_int::MIN||raw.set_errno!=0||raw.delete_result!=c_int::MIN||raw.delete_errno!=0{let mut journal=self.observations.borrow_mut();if journal.first_failure.is_none(){journal.first_failure=Some(raw);}return Err("entry cleanup timer readback unconfirmed".into());}Ok(())
 }
 fn delete_bootstrap(&mut self,cleanup:&ExitTimer)->Probe<()>{
  cleanup.confirmed_until(cleanup.until)?;if self.sticky{return Err("sticky timer cannot be deleted".into());}
  if self.observations.borrow().delete.is_some(){return Err("bootstrap delete cannot be retried".into());}
  let pointer=self.pointer.ok_or("bootstrap timer has no owner")?;let mut raw=TimerObservation::default();unsafe{ptg_entry_timer_delete(pointer.as_ptr(),&mut raw)};
  // Native delete0 FREED the C owner, even if the rest of its receipt is invalid.
  if raw.delete_result==0{self.pointer=None;}
  self.observations.borrow_mut().delete=Some(raw);
  if raw.schema==1&&raw.stage==5&&raw.result==0&&raw.call_errno==0&&raw.delete_result==0&&raw.delete_errno==0&&raw.armed==0&&raw.created==1&&raw.deadline_ns==self.until
   &&raw.create_result==c_int::MIN&&raw.create_errno==0&&raw.set_result==c_int::MIN&&raw.set_errno==0&&raw.get_result==c_int::MIN&&raw.get_errno==0
   &&raw.before_ns==0&&raw.observed_ns==0&&raw.remaining_ns==0&&raw.interval_ns==0{Ok(())}else{let mut journal=self.observations.borrow_mut();if journal.first_failure.is_none(){journal.first_failure=Some(raw);}Err("bootstrap timer delete failed; owner retained".into())}
 }
}
// No Drop deletes/disarms either timer. C sticky ownership also rejects deletion.
// Private, non-Copy original-deadline authority. Only a real sticky timer can mint it.
pub(super) struct OriginalDeadlineProof{timer:Rc<ExitTimer>,metadata:invocation::Metadata,until:u64}
impl OriginalDeadlineProof{
 pub(super) fn bind(original:&invocation::VerifiedInvocation,timer:Rc<ExitTimer>)->Probe<Self>{
  original.check_live().map_err(|e|format!("{e:?}"))?;original.check_time().map_err(|e|format!("{e:?}"))?;
  let until=original.metadata().deadline_ns.checked_add(GRACE).ok_or("cleanup cutoff overflow")?;timer.confirmed_until(until)?;
  Ok(Self{timer,metadata:original.metadata().clone(),until})
 }
 pub(super) fn revalidate(&self,metadata:&invocation::Metadata)->Probe<()>{if metadata!=&self.metadata||metadata.deadline_ns.checked_add(GRACE)!=Some(self.until){return Err("original kernel deadline binding differs".into());}self.timer.confirmed_until(self.until)}
 pub(super) fn until(&self)->u64{self.until}
}
// A captured environment value and pending channel alone never authorize READY or children.
pub(super) struct PendingNotify{authority:supervisor::native_activation::PendingNotifyAuthority,channel:Option<OwnedFd>,sent:bool,attempts:u32,last_result:Option<isize>,last_errno:Option<c_int>}
impl PendingNotify{
 fn capture()->Probe<Self>{
  if std::env::var_os("NOTIFY_SOCKET").as_deref()!=Some(std::ffi::OsStr::new("/run/systemd/notify")){return Err("unexpected PID1 notification endpoint".into());}
  let authority=supervisor::native_activation::PendingNotifyAuthority::capture()?;
  let channel=own_fd(unsafe{socket(AF_UNIX,2|SOCK_NONBLOCK|SOCK_CLOEXEC,0)})?;
  Ok(Self{authority,channel:Some(channel),sent:false,attempts:0,last_result:None,last_errno:None})
 }
 pub(super) fn ready(&mut self,until:u64)->Probe<()>{
  if self.sent||self.attempts!=0||self.channel.is_none(){return Err("notification channel already attempted/consumed".into());}
  if supervisor::boottime_ns()?>=until{return Err("original notification bound expired".into());}self.authority.revalidate()?;
  let(target,length)=address("/run/systemd/notify")?;let fd=self.channel.as_ref().ok_or("notification channel missing")?.as_raw_fd();self.attempts=1;
  let result=unsafe{sendto(fd,b"READY=1".as_ptr().cast(),7,MSG_DONTWAIT|MSG_NOSIGNAL,(&target as *const UnixAddress).cast(),length)};
  self.last_result=Some(result);self.last_errno=if result<0{io::Error::last_os_error().raw_os_error()}else{None};
  if result!=7{return Err(format!("PID1 READY refused result={result} errno={:?}",self.last_errno));}
  self.sent=true;self.authority.revalidate()?;Ok(())
 }
 pub(super) fn consumed(&mut self)->Probe<()>{if !self.sent{return Err("PID1 READY was not sent".into());}self.authority.revalidate()?;drop(self.channel.take());Ok(())}
}

#[repr(C)]struct UnixAddress{family:u16,path:[u8;108]}
fn address(path:&str)->Probe<(UnixAddress,u32)>{if !path.starts_with('/')||path.len()>=108||path.as_bytes().contains(&0){return Err("invalid bounded Unix address".into());}let mut value=UnixAddress{family:AF_UNIX as u16,path:[0;108]};value.path[..path.len()].copy_from_slice(path.as_bytes());Ok((value,(2+path.len()+1) as u32))}
fn own_fd(value:c_int)->Probe<OwnedFd>{let fd=os_result(value as isize).map_err(|e|format!("{e:?}"))? as RawFd;Ok(unsafe{OwnedFd::from_raw_fd(fd)})}
pub(super) fn wait(fd:RawFd,events:c_short,until:u64,owners:Option<&invocation::VerifiedInvocation>)->Probe<()>{
 loop{if let Some(owners)=owners{owners.check_live().map_err(|e|format!("{e:?}"))?;}let now=supervisor::boottime_ns()?;let left=until.checked_sub(now).filter(|v|*v>0).ok_or("native channel deadline expired")?;
  let mut entry=PollFd{fd,events,revents:0};let timeout=left.div_ceil(1_000_000).min(100) as c_int;let count=unsafe{poll(&mut entry,1,timeout)};
  if count<0{return Err(format!("native channel poll: {}",io::Error::last_os_error()));}
  if count==0{continue;}if count!=1||entry.revents&!(events|0x10)!=0{return Err("native channel poll flags refused".into());}if entry.revents&events!=0{return Ok(());}return Err("native channel closed before expected event".into());
 }
}
pub(super) fn connect_root(path:&str,until:u64)->Probe<OwnedFd>{
 let fd=own_fd(unsafe{socket(AF_UNIX,SOCK_SEQPACKET|SOCK_NONBLOCK|SOCK_CLOEXEC,0)})?;
 os_result(unsafe{setsockopt(fd.as_raw_fd(),SOL_SOCKET,SO_PASSCRED,&1,size_of::<c_int>() as u32)} as isize).map_err(|e|format!("{e:?}"))?;
 let (value,length)=address(path)?;let result=unsafe{connect(fd.as_raw_fd(),(&value as *const UnixAddress).cast(),length)};
 if result<0{let error=io::Error::last_os_error().raw_os_error();if !matches!(error,Some(115)|Some(11)){return Err(format!("root socket connect error {error:?}"));}wait(fd.as_raw_fd(),POLLOUT,until,None)?;if socket_option(fd.as_raw_fd(),4).map_err(|e|format!("{e:?}"))?!=0{return Err("root socket completion error".into());}}
 validate_connection(fd.as_raw_fd(),false).map_err(|e|format!("{e:?}"))?;let peer=kernel_peer_credentials(fd.as_raw_fd()).map_err(|e|format!("{e:?}"))?;if peer.uid!=0||peer.gid!=0{return Err("root endpoint is not root".into());}Ok(fd)
}
pub(super) fn send(fd:RawFd,bytes:&[u8],rights:&[RawFd],until:u64)->Probe<()>{loop{match send_packet(fd,bytes,rights){Ok(())=>return Ok(()),Err(Refusal::Os(Some(11)))=>wait(fd,POLLOUT,until,None)?,Err(e)=>return Err(format!("root channel send: {e:?}"))}}}
pub(super) struct Packet{pub(super) bytes:Vec<u8>,pub(super) rights:Vec<OwnedFd>,pub(super) sender:Credentials}
#[derive(Default)]pub(super) struct PacketAttempt{pub(super) recv_attempted:bool,pub(super) returned:Option<isize>,pub(super) errno:Option<c_int>,pub(super) flags:c_int,pub(super) bytes:Vec<u8>}
pub(super) fn packet(fd:RawFd,until:u64,cap:usize)->Probe<Packet>{packet_observed(fd,until,cap,&mut PacketAttempt::default())}
pub(super) fn packet_observed(fd:RawFd,until:u64,cap:usize,receipt:&mut PacketAttempt)->Probe<Packet>{
 wait(fd,POLLIN,until,None)?;let mut bytes=vec![0;cap];let mut iov=IoVec{base:bytes.as_mut_ptr(),len:bytes.len()};let mut control=[0u64;CONTROL_WORDS];let mut msg=message(&mut iov,control.as_mut_ptr().cast(),CONTROL_BYTES);
 receipt.recv_attempted=true;let returned=unsafe{recvmsg(fd,&mut msg,MSG_CMSG_CLOEXEC|MSG_DONTWAIT)};let errno=if returned<0{io::Error::last_os_error().raw_os_error()}else{None};receipt.returned=Some(returned);receipt.errno=errno;receipt.flags=msg.flags;
  let count=if returned<0{return Err(format!("root receive errno {errno:?}"));}else{returned as usize};
 let raw=unsafe{std::slice::from_raw_parts(control.as_ptr().cast::<u8>(),msg.control_len.min(CONTROL_BYTES))};let received=ancillary(raw).map_err(|e|format!("root ancillary: {e:?}"))?;
 // All delivered rights owned before ANY subsequent refusal.
  receipt.bytes=bytes[..count.min(cap)].to_vec();
 if msg.control_len>CONTROL_BYTES||msg.flags&!MSG_CMSG_CLOEXEC!=0||count==0||count>cap||received.unexpected{return Err("root packet framing/flags refused".into());}
 bytes.truncate(count);let sender=received.credential.ok_or("root packet has no kernel credentials")?;Ok(Packet{bytes,rights:received.rights,sender})
}
struct PrivateListener{fd:OwnedFd,path:String,identity:Option<(u64,u64)>,bound:bool,parent_authority:supervisor::native_activation::PrivateRuntimeDirectory,bind_result:c_int,bind_errno:Option<c_int>,path_removed:std::cell::Cell<bool>}
struct ListenerFailure{owner:Option<PrivateListener>,path:String,error:String}
impl PrivateListener{
 fn create(unit:&str)->std::result::Result<Self,ListenerFailure>{
  let path=format!("/run/process-tree-guard/{unit}/private.sock");
  // Hold protected runtime ancestry BEFORE bind/chmod/unlink. No user-writable
  // pathname component is ever followed by a privileged operation.
  let parent_authority=supervisor::native_activation::PrivateRuntimeDirectory::capture(unit).map_err(|error|ListenerFailure{owner:None,path:path.clone(),error})?;
  let fd=own_fd(unsafe{socket(AF_UNIX,SOCK_SEQPACKET|SOCK_NONBLOCK|SOCK_CLOEXEC,0)}).map_err(|error|ListenerFailure{owner:None,path:path.clone(),error})?;
  let mut owner=Self{fd,path,identity:None,bound:false,parent_authority,bind_result:c_int::MIN,bind_errno:None,path_removed:std::cell::Cell::new(false)};
  let result=(||->Probe<()>{
   match fs::symlink_metadata(&owner.path){Err(error)if error.kind()==io::ErrorKind::NotFound=>{},_=>return Err("private socket path exists or cannot be verified absent".into())}
   os_result(unsafe{setsockopt(owner.fd.as_raw_fd(),SOL_SOCKET,SO_PASSCRED,&1,size_of::<c_int>() as u32)} as isize).map_err(|e|format!("{e:?}"))?;
   let (value,length)=address(&owner.path)?;owner.bind_result=unsafe{bind(owner.fd.as_raw_fd(),(&value as *const UnixAddress).cast(),length)};
   if owner.bind_result<0{owner.bind_errno=io::Error::last_os_error().raw_os_error();return Err(format!("private bind errno {:?}; unanchored path retained",owner.bind_errno));}
   owner.bound=true; // Actual post-bind custody exists before metadata/formatting.
   let metadata=fs::symlink_metadata(&owner.path).map_err(|e|e.to_string())?;
   if metadata.uid()!=0||metadata.mode()&0o170000!=0o140000{return Err("new private listener type/owner refused; attempted path retained".into());}
   owner.identity=Some((metadata.dev(),metadata.ino()));
   owner.revalidate_parent()?;owner.require_exact()?;
   fs::set_permissions(&owner.path,fs::Permissions::from_mode(0o622)).map_err(|e|e.to_string())?;
   owner.require_exact()?;os_result(unsafe{listen(owner.fd.as_raw_fd(),1)} as isize).map_err(|e|format!("{e:?}"))?;Ok(())
  })();match result{Ok(())=>Ok(owner),Err(error)=>{let path=owner.path.clone();Err(ListenerFailure{owner:Some(owner),path,error})}}
 }
 fn revalidate_parent(&self)->Probe<()>{self.parent_authority.revalidate()}
 fn require_exact(&self)->Probe<()>{self.revalidate_parent()?;let metadata=fs::symlink_metadata(&self.path).map_err(|e|e.to_string())?;
  if !self.bound||metadata.uid()!=0||metadata.mode()&0o170000!=0o140000||Some((metadata.dev(),metadata.ino()))!=self.identity{return Err("private socket identity changed; quarantined".into());}Ok(())}
 fn accept(&self,until:u64,original:&invocation::VerifiedInvocation)->Probe<OwnedFd>{self.require_exact()?;wait(self.fd.as_raw_fd(),POLLIN,until,Some(original))?;own_fd(unsafe{accept4(self.fd.as_raw_fd(),ptr::null_mut(),ptr::null_mut(),SOCK_NONBLOCK|SOCK_CLOEXEC)})}
 fn unlink_exact(&self)->Probe<()>{if self.path_removed.get()||!self.bound{return Ok(());}self.require_exact()?;fs::remove_file(&self.path).map_err(|e|e.to_string())?;self.path_removed.set(true);Ok(())}
 fn cleanup_error(&self,primary:&str)->String{match self.unlink_exact(){Ok(())=>primary.to_owned(),Err(error)=>format!("{primary}; exact private socket cleanup: {error}; original owner retained for PID1 runtime cleanup")}}
}
#[derive(Clone,Copy,Debug)]
pub(super) struct CancellationObservation{pub(super) returned:c_int,pub(super) errno:c_int,pub(super) revents:c_short}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) enum CancellationState{Clear,Peer,Fault}
impl CancellationObservation{
 pub(super) fn state(self)->CancellationState{
  if self.returned==0&&self.errno==0&&self.revents==0{return CancellationState::Clear;}
  if self.returned==1&&self.errno==0&&self.revents!=0&&self.revents&!(POLLIN|0x2000|0x10)==0{return CancellationState::Peer;}
  CancellationState::Fault
 }
}
// Constructor/fields stay private to the actual service entry. This is the
// accepted final connection bound to the original requester, never a raw-FD token.
pub(crate) struct AuthenticatedCancellation{fd:OwnedFd,peer:Credentials}
impl AuthenticatedCancellation{
 fn from_private(private:&OwnedFd,original:&invocation::VerifiedInvocation,peer:Credentials)->Probe<Self>{
  validate_connection(private.as_raw_fd(),true).map_err(|e|format!("private cancellation channel: {e:?}"))?;
  if kernel_peer_credentials(private.as_raw_fd()).map_err(|e|format!("{e:?}"))?!=peer||original.metadata().identities.first().map(|identity|identity.pid as c_int)!=Some(peer.pid){return Err("cancellation peer differs from original requester".into());}
  validate_pidfd(original.requester(),peer.pid,&mut NativeReceipt::default()).map_err(|e|format!("cancellation requester: {e:?}"))?;
  let fd=own_fd(unsafe{fcntl(private.as_raw_fd(),1030,3 as c_int)})?;let owner=Self{fd,peer};
  validate_connection(owner.fd.as_raw_fd(),true).map_err(|e|format!("owned cancellation channel: {e:?}"))?;
  if kernel_peer_credentials(owner.fd.as_raw_fd()).map_err(|e|format!("{e:?}"))?!=peer{return Err("duplicated cancellation peer differs".into());}Ok(owner)
 }
 pub(super) fn fd(&self)->RawFd{self.fd.as_raw_fd()}
 pub(super) fn peer(&self)->Credentials{self.peer}
 pub(super) fn shutdown_read(&self)->(c_int,c_int){let result=unsafe{shutdown(self.fd(),0)};let errno=if result<0{io::Error::last_os_error().raw_os_error().unwrap_or(0)}else{0};(result,errno)}
 pub(super) fn sample(&self)->CancellationObservation{
  let mut event=PollFd{fd:self.fd(),events:POLLIN|0x2000,revents:0};let returned=unsafe{poll(&mut event,1,0)};let errno=if returned<0{io::Error::last_os_error().raw_os_error().unwrap_or(0)}else{0};CancellationObservation{returned,errno,revents:event.revents}
 }
}

#[derive(Clone)]pub(super) struct Hello{pub(super) pid:u32,pub(super) invocation:[u8;16],pub(super) cutoff:u64,pub(super) unit:String,pub(super) group:String,pub(super) socket:String,pub(super) early:i32}
impl Hello{
 fn encode(&self)->Probe<Vec<u8>>{let mut out=b"PTGBOOT1".to_vec();out.extend_from_slice(&self.pid.to_le_bytes());out.extend_from_slice(&self.invocation);out.extend_from_slice(&self.cutoff.to_le_bytes());out.extend_from_slice(&self.early.to_le_bytes());for text in [&self.unit,&self.group,&self.socket]{if text.len()>4096{return Err("hello field bound".into());}out.extend_from_slice(&(text.len() as u16).to_le_bytes());out.extend_from_slice(text.as_bytes());}Ok(out)}
 pub(super) fn decode(bytes:&[u8])->Probe<Self>{if bytes.len()<40||&bytes[..8]!=b"PTGBOOT1"{return Err("root hello version".into());}let pid=u32::from_le_bytes(bytes[8..12].try_into().expect("fixed"));let invocation=bytes[12..28].try_into().expect("fixed");let cutoff=u64::from_le_bytes(bytes[28..36].try_into().expect("fixed"));let early=i32::from_le_bytes(bytes[36..40].try_into().expect("fixed"));let mut rest=&bytes[40..];let mut texts=Vec::new();for _ in 0..3{let (size,tail)=rest.split_first_chunk::<2>().ok_or("hello length")?;let n=usize::from(u16::from_le_bytes(*size));if n>4096{return Err("hello bound".into());}let (text,tail)=tail.split_at_checked(n).ok_or("hello truncation")?;texts.push(String::from_utf8(text.to_vec()).map_err(|_|"hello UTF8")?);rest=tail;}if !rest.is_empty()||pid==0||invocation==[0;16]||!matches!(early,0|124|125){return Err("hello facts refused".into());}Ok(Self{pid,invocation,cutoff,unit:texts.remove(0),group:texts.remove(0),socket:texts.remove(0),early})}
}
pub(super) fn outcome(invocation:[u8;16],code:i32,settled:bool,timeout:Option<TimeoutCause>,original_deadline:u64)->Vec<u8>{let mut frame=b"PTGOUTC2".to_vec();frame.extend_from_slice(&invocation);frame.extend_from_slice(&code.to_le_bytes());frame.extend_from_slice(&u32::from(settled).to_le_bytes());frame.push(match timeout{None=>0,Some(TimeoutCause::PreLaunch)=>1,Some(TimeoutCause::Arming)=>2,Some(TimeoutCause::Terminal)=>3});frame.extend_from_slice(&if timeout.is_some(){original_deadline}else{0}.to_le_bytes());frame}


fn adopted_activation()->Probe<OwnedFd>{
 // Adopt FD3 before opening ANY other FD. LISTEN_* is not an origin authority.
 os_result(unsafe{fcntl(3,F_GETFD)} as isize).map_err(|e|format!("missing activation FD: {e:?}"))?;
 let fd=unsafe{OwnedFd::from_raw_fd(3)};
 os_result(unsafe{fcntl(3,2,FD_CLOEXEC)} as isize).map_err(|e|format!("activation CLOEXEC: {e:?}"))?;
 let flags=os_result(unsafe{fcntl(3,F_GETFL)} as isize).map_err(|e|format!("activation flags: {e:?}"))? as c_int;
 os_result(unsafe{fcntl(3,4,flags|SOCK_NONBLOCK)} as isize).map_err(|e|format!("activation nonblocking: {e:?}"))?;
 validate_connection(3,true).map_err(|e|format!("activation socket: {e:?}"))?;
 let mut local=UnixAddress{family:0,path:[0;108]};let mut length=size_of::<UnixAddress>() as u32;
 os_result(unsafe{getsockname(3,(&mut local as *mut UnixAddress).cast(),&mut length)} as isize).map_err(|e|format!("activation name: {e:?}"))?;
 let n=local.path.iter().position(|v|*v==0).ok_or("unterminated activation path")?;
 if local.family!=AF_UNIX as u16||&local.path[..n]!=manager::ACTIVATE.as_bytes(){return Err("activation local path differs".into());}Ok(fd)
}
fn erase_service_environment()->Probe<()>{
 let vars:Vec<_>=std::env::vars_os().collect();if vars.len()>256||vars.iter().map(|(k,v)|k.len()+v.len()).sum::<usize>()>65536{return Err("service environment exceeds bootstrap bound".into());}
 // Dedicated fresh root service only; never used by the library client.
 for (name,_) in vars{unsafe{std::env::remove_var(name)};}Ok(())
}
fn append_timer(frame:&mut Vec<u8>,timer:&ExitTimer){
 let log=timer.observations.borrow();frame.push(u8::from(timer.sticky));for word in timer.abi{frame.extend_from_slice(&word.to_le_bytes());}frame.extend_from_slice(&log.checks.to_le_bytes());
 for slot in [Some(log.create),log.latest,log.first_failure,log.delete]{frame.push(u8::from(slot.is_some()));if let Some(raw)=slot{
  for n in [raw.schema,raw.stage]{frame.extend_from_slice(&n.to_le_bytes());}
  for n in [raw.result,raw.call_errno,raw.created,raw.armed,raw.create_result,raw.create_errno,raw.set_result,raw.set_errno,raw.get_result,raw.get_errno,raw.delete_result,raw.delete_errno]{frame.extend_from_slice(&n.to_le_bytes());}
  for n in [raw.deadline_ns,raw.observed_ns,raw.remaining_ns,raw.interval_ns,raw.before_ns]{frame.extend_from_slice(&n.to_le_bytes());}
 }}
}
fn root_frame(invocation:[u8;16],code:i32,settled:bool,error:&str,timeout:Option<TimeoutCause>,original_deadline:u64,bootstrap:&ExitTimer,cleanup:Option<&ExitTimer>)->Vec<u8>{
 let mut frame=outcome(invocation,code,settled,timeout,original_deadline);let bytes=error.as_bytes();let mut n=bytes.len().min(1024);while !error.is_char_boundary(n){n-=1;}frame.extend_from_slice(&(n as u16).to_le_bytes());frame.extend_from_slice(&bytes[..n]);frame.push(if cleanup.is_some(){2}else{1});append_timer(&mut frame,bootstrap);if let Some(timer)=cleanup{append_timer(&mut frame,timer);}frame
}

// Private root-to-client protocol: bounded, exact, and losslessly retained.
struct WireCursor<'a>{rest:&'a[u8]}
impl<'a> WireCursor<'a>{
 fn take<const N:usize>(&mut self)->Probe<[u8;N]>{let (value,rest)=self.rest.split_first_chunk::<N>().ok_or("root result truncated")?;self.rest=rest;Ok(*value)}
 fn byte(&mut self)->Probe<u8>{Ok(self.take::<1>()?[0])}
 fn u32(&mut self)->Probe<u32>{Ok(u32::from_le_bytes(self.take()?))}
 fn i32(&mut self)->Probe<i32>{Ok(i32::from_le_bytes(self.take()?))}
 fn u64(&mut self)->Probe<u64>{Ok(u64::from_le_bytes(self.take()?))}
 fn observation(&mut self)->Probe<TimerObservation>{Ok(TimerObservation{schema:self.u32()?,stage:self.u32()?,result:self.i32()?,call_errno:self.i32()?,created:self.i32()?,armed:self.i32()?,create_result:self.i32()?,create_errno:self.i32()?,set_result:self.i32()?,set_errno:self.i32()?,get_result:self.i32()?,get_errno:self.i32()?,delete_result:self.i32()?,delete_errno:self.i32()?,deadline_ns:self.u64()?,observed_ns:self.u64()?,remaining_ns:self.u64()?,interval_ns:self.u64()?,before_ns:self.u64()?})}
 fn optional_observation(&mut self)->Probe<Option<TimerObservation>>{match self.byte()?{0=>Ok(None),1=>self.observation().map(Some),_=>Err("root timer slot marker invalid".into())}}
}
struct TimerEvidence{sticky:bool,abi:[u64;9],checks:u64,create:TimerObservation,latest:Option<TimerObservation>,first_failure:Option<TimerObservation>,delete:Option<TimerObservation>}
pub(super) struct DecodedOutcome{pub(super) code:i32,pub(super) settled:bool,pub(super) timeout:Option<TimeoutCause>,pub(super) error:String,timers:Vec<TimerEvidence>}
fn created_timer(raw:&TimerObservation)->bool{timer_readback(raw,raw.deadline_ns)&&raw.create_result==0&&raw.create_errno==0&&raw.set_result==0&&raw.set_errno==0&&raw.delete_result==c_int::MIN&&raw.delete_errno==0}
fn observed_timer(raw:&TimerObservation,until:u64)->bool{timer_readback(raw,until)&&raw.create_result==c_int::MIN&&raw.create_errno==0&&raw.set_result==c_int::MIN&&raw.set_errno==0&&raw.delete_result==c_int::MIN&&raw.delete_errno==0}
fn deleted_timer(raw:&TimerObservation,until:u64)->bool{raw.schema==1&&raw.stage==5&&raw.result==0&&raw.call_errno==0&&raw.created==1&&raw.armed==0&&raw.delete_result==0&&raw.delete_errno==0&&raw.deadline_ns==until&&raw.create_result==c_int::MIN&&raw.create_errno==0&&raw.set_result==c_int::MIN&&raw.set_errno==0&&raw.get_result==c_int::MIN&&raw.get_errno==0&&raw.before_ns==0&&raw.observed_ns==0&&raw.remaining_ns==0&&raw.interval_ns==0}
impl DecodedOutcome{
 pub(super) fn decode(bytes:&[u8],invocation:[u8;16],cleanup_until:u64,original_deadline:u64)->Probe<Self>{
  let mut cursor=WireCursor{rest:bytes};if cursor.take::<8>()?!=*b"PTGOUTC2"||cursor.take::<16>()?!=invocation{return Err("root result identity/version differs".into());}
  let code=cursor.i32()?;let settled=match cursor.u32()?{0=>false,1=>true,_=>return Err("root settlement flag invalid".into())};if !(0..=255).contains(&code){return Err("root result exit code invalid".into());}
  let timeout=match cursor.byte()?{0=>None,1=>Some(TimeoutCause::PreLaunch),2=>Some(TimeoutCause::Arming),3=>Some(TimeoutCause::Terminal),_=>return Err("root timeout cause invalid".into())};let reported_deadline=cursor.u64()?;
  let length=usize::from(u16::from_le_bytes(cursor.take()?));if length>1024{return Err("root result error bound".into());}let (error,rest)=cursor.rest.split_at_checked(length).ok_or("root result error truncated")?;let error=std::str::from_utf8(error).map_err(|_|"root result error UTF8")?.to_owned();cursor.rest=rest;
  if timeout.is_some(){if code!=124||!settled||!error.is_empty()||original_deadline==0||reported_deadline!=original_deadline||original_deadline.checked_add(GRACE)!=Some(cleanup_until){return Err("root timeout cause/deadline differs".into());}}else if reported_deadline!=0{return Err("non-timeout result contains deadline".into());}
  let count=cursor.byte()?;if !(1..=2).contains(&count){return Err("root timer count invalid".into());}let mut timers=Vec::with_capacity(usize::from(count));
  for index in 0..count{let sticky=match cursor.byte()?{0=>false,1=>true,_=>return Err("root timer role invalid".into())};if sticky!=(index==1){return Err("root timer role/order differs".into());}let mut abi=[0;9];for word in &mut abi{*word=cursor.u64()?;}let checks=cursor.u64()?;let create=cursor.optional_observation()?.ok_or("root timer creation receipt missing")?;let latest=cursor.optional_observation()?;let first_failure=cursor.optional_observation()?;let delete=cursor.optional_observation()?;timers.push(TimerEvidence{sticky,abi,checks,create,latest,first_failure,delete});}
  if !cursor.rest.is_empty(){return Err("root result has trailing bytes".into());}
  let value=Self{code,settled,timeout,error,timers};
  // A positive completion can only come from the full bootstrap-to-sticky handoff.
  // Failed receipts remain available in the raw frame, never promoted to success.
  if value.settled{
   if value.timers.len()!=2{return Err("completed result lacks both timer authorities".into());}
   for timer in &value.timers{if timer.abi!=[1,96,7,9,0,1,64,32,56]||!created_timer(&timer.create)||timer.first_failure.is_some()||timer.latest.is_some()!= (timer.checks!=0){return Err("completed timer evidence invalid".into());}if let Some(raw)=&timer.latest{if !observed_timer(raw,timer.create.deadline_ns){return Err("completed timer observation invalid".into());}}}
   let bootstrap=&value.timers[0];let cleanup=&value.timers[1];
   if bootstrap.sticky||!cleanup.sticky||cleanup.create.deadline_ns!=cleanup_until||cleanup.latest.is_none()||cleanup.delete.is_some()||!bootstrap.delete.as_ref().is_some_and(|raw|deleted_timer(raw,bootstrap.create.deadline_ns)){return Err("completed timer handoff/cleanup evidence invalid".into());}
  }
  Ok(value)
 }
}

// Dedicated process diagnostics only. Native sticky/bootstrap/PID1 authority is
// still armed; no library host stream flags or environment are modified.
fn write_diagnostic(bytes:&[u8]){
 // Never F_SETFL on inherited stderr: its OFD may also belong to the parent.
 // The dedicated process's independently armed self timer bounds the write,
 // including a readiness race or a slow regular-file/device write.
 let mut ready=PollFd{fd:2,events:POLLOUT,revents:0};if unsafe{poll(&mut ready,1,0)}!=1||ready.revents!=POLLOUT{return;}
 let _raw_write=unsafe{write(2,bytes.as_ptr().cast(),bytes.len().min(2048))};
}
fn report_failure(error:&str){let text=format!("process-tree-guard: {error}\n");write_diagnostic(text.as_bytes());}
fn timeout_notice(cause:ClientTimeout)->&'static [u8]{match cause{
 ClientTimeout::Joined(_)=>b"PROCESS_TIMEOUT: original invocation deadline expired\n",
 ClientTimeout::NoLaunch=>b"PROCESS_TIMEOUT: original invocation deadline expired before activation; no payload launched\n",
}}
fn refuse_activation(fd:RawFd,error:&str,bootstrap:&ExitTimer,cleanup:Option<&ExitTimer>)->!{
 let frame=root_frame([0;16],125,false,error,None,0,bootstrap,cleanup);
 let until=cleanup.map_or(bootstrap.until,|timer|timer.until.min(bootstrap.until));
 let send_result=send(fd,&frame,&[],until);report_failure(error);if let Err(error)=send_result{report_failure(&error);}
 unsafe{_exit(125)}
}
pub(super) fn cli_timer(until:u64)->std::result::Result<Rc<ExitTimer>,(Rc<ExitTimer>,String)>{ExitTimer::create(until,true).map_err(|failure|(failure.owner,failure.error))}
pub(super) fn cli_exit(code:i32,error:Option<&str>,timer:Option<&ExitTimer>,timeout:Option<ClientTimeout>)->!{
 let _early_exit_authority=if let Some(timer)=timer{if timer.confirmed_until(timer.until).is_err(){unsafe{_exit(125)}}None}else{
  // Only pre-capture/usage refusals lack the original standalone authority.
  let until=match supervisor::boottime_ns().ok().and_then(|now|now.checked_add(1_000_000_000)){Some(value)=>value,None=>unsafe{_exit(125)}};
  match ExitTimer::create(until,true){Ok(owner)=>Some(owner),Err(failure)=>{let _retained=failure.owner;unsafe{_exit(125)}}}
 };
 if let Some(cause)=timeout{write_diagnostic(timeout_notice(cause));}else if let Some(error)=error{report_failure(error);}unsafe{_exit(code)}
}

/// Actual dedicated root binary entry. It never returns into an arbitrary host.
/// Source-level authority is established below; there is no unsafe origin mint.
pub(crate) fn root_service_main()->!{
 if unsafe{getuid()}!=0||unsafe{geteuid()}!=0||unsafe{getgid()}!=0||unsafe{getegid()}!=0{unsafe{_exit(125)}}
 let args:Vec<_>=std::env::args_os().collect();if args.len()!=2||args[1]!="--service"{unsafe{_exit(125)}}
 let activation=match adopted_activation(){Ok(fd)=>fd,Err(_)=>unsafe{_exit(125)}};
 let now=match supervisor::boottime_ns(){Ok(n)=>n,Err(_)=>unsafe{_exit(125)}};
 let until=match now.checked_add(BOOTSTRAP){Some(n)=>n,None=>unsafe{_exit(125)}};
 let mut bootstrap=match ExitTimer::create(until,false){Ok(owner)=>owner,Err(failure)=>{let _retained=failure.owner;unsafe{_exit(125)}}};
 let mut notification=match PendingNotify::capture(){Ok(owner)=>owner,Err(error)=>refuse_activation(activation.as_raw_fd(),&error,&bootstrap,None)};
 if let Err(error)=erase_service_environment(){refuse_activation(activation.as_raw_fd(),&error,&bootstrap,None)}
 let mut cleanup:Option<Rc<ExitTimer>>=None;
 let mut admission=NativeReceipt::default();
 if let Err(error)=wait(activation.as_raw_fd(),POLLIN,until,None){refuse_activation(activation.as_raw_fd(),&error,&bootstrap,None)}
 let first=receive_entry(activation.as_raw_fd(),&mut admission,&mut |deadline|{
  let end=deadline.checked_add(GRACE).ok_or_else(||Refusal::Entry("cleanup deadline overflow".into()))?;
  match ExitTimer::create(end,true){Ok(owner)=>{cleanup=Some(owner);Ok(())},Err(failure)=>{cleanup=Some(failure.owner);Err(Refusal::Entry(failure.error))}}
 });
 let first=match first{Ok(first)=>first,Err(error)=>refuse_activation(activation.as_raw_fd(),&format!("initial admission: {error:?}"),&bootstrap,cleanup.as_deref())};
 if let Err(error)=supervisor::validate_received_identity(&first){refuse_activation(activation.as_raw_fd(),&error,&bootstrap,cleanup.as_deref())}
 let cleanup=match cleanup{Some(owner)=>owner,None=>refuse_activation(activation.as_raw_fd(),"missing cleanup self timer",&bootstrap,None)};
 let mut scope=match supervisor::native_activation::capture_service_scope(){Ok(scope)=>scope,Err(error)=>refuse_activation(activation.as_raw_fd(),&error,&bootstrap,Some(&cleanup))};
 if let Err(error)=scope.bind_delegation(first.peer.uid,first.invocation.metadata()){refuse_activation(activation.as_raw_fd(),&error,&bootstrap,Some(&cleanup))}
 scope=match scope.bind_original(&first.invocation,Rc::clone(&cleanup)){Ok(scope)=>scope,Err((held,error))=>{let _retained=held;refuse_activation(activation.as_raw_fd(),&error,&bootstrap,Some(&cleanup))}};
 if let Err(error)=scope.enter_running(&mut notification,&first.invocation,until){refuse_activation(activation.as_raw_fd(),&error,&bootstrap,Some(&cleanup))}
 let (unit,group,invocation,service_pidfd,events)=match scope.completion_capabilities(){Ok(info)=>info,Err(error)=>refuse_activation(activation.as_raw_fd(),&error,&bootstrap,Some(&cleanup))};
 let listener=match PrivateListener::create(unit.to_str().unwrap_or("")){Ok(value)=>value,Err(failure)=>{let retained=failure.owner;let _attempted_path=failure.path;let primary=match retained.as_ref(){Some(owner)=>owner.cleanup_error(&failure.error),None=>failure.error};let _quarantine=retained;refuse_activation(activation.as_raw_fd(),&primary,&bootstrap,Some(&cleanup))}};
 let hello=Hello{pid:std::process::id(),invocation,cutoff:cleanup.until,unit:unit.to_string_lossy().into_owned(),group,socket:listener.path.clone(),early:0};
 let bytes=match hello.encode(){Ok(bytes)=>bytes,Err(error)=>{let error=listener.cleanup_error(&error);refuse_activation(activation.as_raw_fd(),&error,&bootstrap,Some(&cleanup))}};
 if let Err(error)=send(activation.as_raw_fd(),&bytes,&[service_pidfd.as_raw_fd(),events.as_raw_fd()],until){let error=listener.cleanup_error(&error);refuse_activation(activation.as_raw_fd(),&error,&bootstrap,Some(&cleanup))}
 let private=match listener.accept(until.min(first.invocation.metadata().deadline_ns),&first.invocation){Ok(fd)=>fd,Err(error)=>{let _primary=error;let cleanup_result=listener.unlink_exact();let _quarantine=listener;let error=match cleanup_result{Ok(())=>_primary,Err(cleanup_error)=>format!("{_primary}; exact socket cleanup: {cleanup_error}")};refuse_activation(activation.as_raw_fd(),&error,&bootstrap,Some(&cleanup))}};
 let result=(||->Probe<()>{
  first.invocation.check_live().map_err(|e|format!("{e:?}"))?;
  let peer=kernel_peer_credentials(private.as_raw_fd()).map_err(|e|format!("{e:?}"))?;if peer!=first.peer{return Err("private socket caller differs from original held requester".into());}
  wait(private.as_raw_fd(),POLLIN,until.min(first.invocation.metadata().deadline_ns),Some(&first.invocation))?;
  let mut again=NativeReceipt::default();let second=receive_observed(private.as_raw_fd(),&mut again).map_err(|e|format!("private admission: {e:?}"))?;
  if second.peer!=first.peer||second.invocation.metadata()!=first.invocation.metadata()||second.plan.arguments()!=first.plan.arguments()||second.plan.environment()!=first.plan.environment(){return Err("private request differs from captured bootstrap request".into());}
  first.invocation.check_live().map_err(|e|format!("{e:?}"))?;first.invocation.check_time().map_err(|e|format!("{e:?}"))?;
  drop(second);listener.unlink_exact()?;scope.revalidate()?;
  Rc::get_mut(&mut bootstrap).ok_or("bootstrap timer unexpectedly shared")?.delete_bootstrap(&cleanup)?;Ok(())
 })();
 if let Err(error)=result{let error=listener.cleanup_error(&error);report_failure(&error);let frame=root_frame(invocation,125,false,&error,None,0,&bootstrap,Some(&cleanup));let _=send(private.as_raw_fd(),&frame,&[],cleanup.until);unsafe{_exit(125)}}
 // All original authenticated IO/cwd/environment and exact lease feed the
 // existing real controller. No rewritten/ambient caller context is introduced.
 let cancellation=match AuthenticatedCancellation::from_private(&private,&first.invocation,first.peer){Ok(value)=>std::sync::Arc::new(value),Err(error)=>{let frame=root_frame(invocation,125,false,&error,None,0,&bootstrap,Some(&cleanup));let _send=send(private.as_raw_fd(),&frame,&[],cleanup.until);unsafe{_exit(125)}}};
 let original_deadline=first.invocation.metadata().deadline_ns;
 let run=unsafe{supervisor::run_admitted_in_service(scope,first,admission,cancellation,|uid,gid,groups,program|uid!=0&&gid!=0&&!groups.contains(&0)&&program.to_bytes().starts_with(b"/"))};
 let mut classified=run.classify();if !run.settled(){classified=RunClassification::failure("native settlement is quarantined".into());}
 if let Err(failure)=run.validate_final_scope(){classified=RunClassification::failure(failure);}
 let code=classified.code;let error=classified.error.as_deref().unwrap_or("");
 let frame=root_frame(invocation,code,run.settled(),error,classified.timeout,original_deadline,&bootstrap,Some(&cleanup));
 if let Err(error)=send(private.as_raw_fd(),&frame,&[],cleanup.until){report_failure(&error);unsafe{_exit(125)}}
 // No exec, destructor flush, timer disarm or unbounded diagnostic tail follows.
 // Native owners/namespace/unit/timers stay live until the kernel closes them.
 unsafe{_exit(code)}
}

#[cfg(test)]pub(super) fn original_deadline_readback_regression(){
 let until=1000;let mut raw=TimerObservation{schema:1,stage:4,result:0,call_errno:0,created:1,armed:1,get_result:0,get_errno:0,deadline_ns:until,remaining_ns:100,interval_ns:0,before_ns:899,observed_ns:901,..TimerObservation::default()};
 assert!(timer_readback(&raw,until));assert!(!timer_readback(&raw,until+1));
 raw.interval_ns=1;assert!(!timer_readback(&raw,until));raw.interval_ns=0;raw.remaining_ns=0;assert!(!timer_readback(&raw,until));raw.remaining_ns=100;raw.before_ns=902;assert!(!timer_readback(&raw,until));raw.before_ns=899;raw.get_errno=5;assert!(!timer_readback(&raw,until));
 timeout_wire_regression();
 eprintln!("notify_original_timer_raw_readback_rejections_verified");
}

#[cfg(test)]fn timeout_wire_regression(){
 // Synthetic records exercise the production serializer/parser only; no timer
 // owner, native settlement, cgroup or invocation authority is minted here.
 let original=10_000;let cleanup_until=original+GRACE;let invocation=[7;16];
 let created=|until|TimerObservation{schema:1,stage:4,result:0,call_errno:0,created:1,armed:1,create_result:0,create_errno:0,set_result:0,set_errno:0,get_result:0,get_errno:0,deadline_ns:until,remaining_ns:100,before_ns:until-101,observed_ns:until-99,..TimerObservation::default()};
 let bootstrap_until=2000;let deleted=TimerObservation{schema:1,stage:5,result:0,call_errno:0,created:1,armed:0,delete_result:0,delete_errno:0,deadline_ns:bootstrap_until,..TimerObservation::default()};
 let bootstrap=ExitTimer{abi:[1,96,7,9,0,1,64,32,56],pointer:None,sticky:false,until:bootstrap_until,observations:RefCell::new(TimerJournal{create:created(bootstrap_until),latest:None,first_failure:None,delete:Some(deleted),checks:0})};
 let mut latest=created(cleanup_until);latest.create_result=c_int::MIN;latest.set_result=c_int::MIN;
 let cleanup=ExitTimer{abi:[1,96,7,9,0,1,64,32,56],pointer:None,sticky:true,until:cleanup_until,observations:RefCell::new(TimerJournal{create:created(cleanup_until),latest:Some(latest),first_failure:None,delete:None,checks:1})};
 for cause in [TimeoutCause::PreLaunch,TimeoutCause::Arming,TimeoutCause::Terminal]{let bytes=root_frame(invocation,124,true,"",Some(cause),original,&bootstrap,Some(&cleanup));let decoded=DecodedOutcome::decode(&bytes,invocation,cleanup_until,original).unwrap();assert_eq!(decoded.timeout,Some(cause));
  assert!(DecodedOutcome::decode(&bytes,invocation,cleanup_until,cleanup_until).is_err());assert!(DecodedOutcome::decode(&bytes,[8;16],cleanup_until,original).is_err());
  for n in 0..bytes.len(){assert!(DecodedOutcome::decode(&bytes[..n],invocation,cleanup_until,original).is_err());}
  for (offset,value) in [(0,b'X'),(7,b'1'),(32,255),(33,0)]{let mut bad=bytes.clone();bad[offset]=value;assert!(DecodedOutcome::decode(&bad,invocation,cleanup_until,original).is_err());}
  let mut trailing=bytes.clone();trailing.push(0);assert!(DecodedOutcome::decode(&trailing,invocation,cleanup_until,original).is_err());
  for (code,settled,error) in [(125,true,""),(124,false,""),(124,true,"fault")]{let bad=root_frame(invocation,code,settled,error,Some(cause),original,&bootstrap,Some(&cleanup));assert!(DecodedOutcome::decode(&bad,invocation,cleanup_until,original).is_err());}
 }
 for code in [124,125]{let bytes=root_frame(invocation,code,true,"",None,original,&bootstrap,Some(&cleanup));assert_eq!(DecodedOutcome::decode(&bytes,invocation,cleanup_until,original).unwrap().timeout,None);let mut bad=bytes;bad[33..41].copy_from_slice(&original.to_le_bytes());assert!(DecodedOutcome::decode(&bad,invocation,cleanup_until,original).is_err());}
 assert_eq!(timeout_notice(ClientTimeout::Joined(TimeoutCause::Terminal)),b"PROCESS_TIMEOUT: original invocation deadline expired\n");assert!(timeout_notice(ClientTimeout::NoLaunch).len()<128);
 eprintln!("typed_timeout_PTGOUTC2_original_deadline_negative_controls_verified");
}
