// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
//! Actual bounded libsystemd calls; no shell command or supplied property snapshot.
use super::*;
use std::ffi::{CStr,CString,c_char};
use std::ptr::NonNull;
const DEST:&CStr=c"org.freedesktop.systemd1";
const MANAGER:&CStr=c"org.freedesktop.systemd1.Manager";
const UNIT:&CStr=c"org.freedesktop.systemd1.Unit";
const SERVICE:&CStr=c"org.freedesktop.systemd1.Service";
const ROOT:&CStr=c"/org/freedesktop/systemd1";
pub(super) const HELPER:&str="/usr/libexec/process-tree-guard-root";
pub(super) const SOCKET_FILE:&str="/usr/lib/systemd/system/process-tree-guard.socket";
pub(super) const TEMPLATE_FILE:&str="/usr/lib/systemd/system/process-tree-guard@.service";
pub(super) const ACTIVATE:&str="/run/process-tree-guard/activate.sock";
const PREFIX:&str="process-tree-guard@";
type Probe<T>=std::result::Result<T,String>;
#[repr(C)]struct SdBus{_opaque:[u8;0]}
#[repr(C)]struct SdMessage{_opaque:[u8;0]}
#[repr(C)]struct SdBusError{name:*const c_char,message:*const c_char,_need_free:c_int}
struct BusError(SdBusError);
impl BusError{
 fn new()->Self{Self(SdBusError{name:ptr::null(),message:ptr::null(),_need_free:0})}
}
impl Drop for BusError{fn drop(&mut self){unsafe{sd_bus_error_free(&mut self.0);}}}
fn escaped_error_bytes(bytes:&[u8],truncated:bool)->String{
 let mut value=String::from("\"");
 for &byte in bytes{if (0x20..=0x7e).contains(&byte)&&byte!=b'"'&&byte!=b'\\'{value.push(char::from(byte));}else{value.push_str(&format!("\\x{byte:02x}"));}}
 value.push('"');if truncated{value.push_str("<truncated>");}value
}
fn bounded_error_field(pointer:*const c_char,limit:usize)->String{
 if pointer.is_null(){return "<null>".into();}
 let mut bytes=Vec::with_capacity(limit);
 // libsystemd owns a valid NUL-terminated string until BusError is dropped.
 // Stop at NUL or the fixed copy limit; never scan the full error string.
 for index in 0..limit{let byte=unsafe{pointer.add(index).read()} as u8;if byte==0{return escaped_error_bytes(&bytes,false);}bytes.push(byte);}
 escaped_error_bytes(&bytes,true)
}
fn bounded_error_argument(value:&CStr,limit:usize)->String{let bytes=value.to_bytes();escaped_error_bytes(&bytes[..bytes.len().min(limit)],bytes.len()>limit)}
fn reference_operation(member:&CStr,name:&CStr,error:&SdBusError)->String{
 let mut value=format!("unit reference operation member={} unit={} error_name={} error_message={}",bounded_error_argument(member,16),bounded_error_argument(name,255),bounded_error_field(error.name,256),bounded_error_field(error.message,1024));
 if value.len()>8192{value.truncate(8176);value.push_str("<truncated>");}value
}

#[link(name="libsystemd.so.0",kind="dylib",modifiers="+verbatim")]
unsafe extern "C" {
 fn sd_bus_new(bus:*mut *mut SdBus)->c_int;
 fn sd_bus_set_address(bus:*mut SdBus,address:*const c_char)->c_int;
 fn sd_bus_set_bus_client(bus:*mut SdBus,client:c_int)->c_int;
 fn sd_bus_start(bus:*mut SdBus)->c_int;
 fn sd_bus_get_fd(bus:*mut SdBus)->c_int;
 fn sd_bus_set_method_call_timeout(bus:*mut SdBus,usec:u64)->c_int;
 fn sd_bus_unref(bus:*mut SdBus)->*mut SdBus;
 fn sd_bus_message_unref(message:*mut SdMessage)->*mut SdMessage;
 fn sd_bus_error_free(error:*mut SdBusError);
 fn sd_bus_call_method(bus:*mut SdBus,destination:*const c_char,path:*const c_char,interface:*const c_char,member:*const c_char,error:*mut SdBusError,reply:*mut *mut SdMessage,types:*const c_char,...)->c_int;
 fn sd_bus_get_property(bus:*mut SdBus,destination:*const c_char,path:*const c_char,interface:*const c_char,member:*const c_char,error:*mut c_void,reply:*mut *mut SdMessage,kind:*const c_char)->c_int;
 fn sd_bus_message_read_basic(message:*mut SdMessage,kind:c_char,value:*mut c_void)->c_int;
 fn sd_bus_message_read_array(message:*mut SdMessage,kind:c_char,value:*mut *const c_void,size:*mut usize)->c_int;
 fn sd_bus_message_enter_container(message:*mut SdMessage,kind:c_char,contents:*const c_char)->c_int;
 fn sd_bus_message_skip(message:*mut SdMessage,types:*const c_char)->c_int;
 fn sd_bus_message_exit_container(message:*mut SdMessage)->c_int;
}
fn call(result:c_int,operation:&str)->Probe<c_int>{if result<0{Err(format!("{operation}: libsystemd error {result}"))}else{Ok(result)}}
struct Message(NonNull<SdMessage>);
fn owned_reply(result:c_int,raw:*mut SdMessage,operation:&str)->Probe<Message>{
 let owned=NonNull::new(raw).map(Message); // adopt even a returned error reply
 if result<0{drop(owned);return Err(format!("{operation}: libsystemd error {result}"));}
 owned.ok_or_else(||format!("{operation}: missing systemd reply"))
}
impl Message{
 fn basic<T:Default>(&self,kind:u8)->Probe<T>{let mut value=T::default();let count=call(unsafe{sd_bus_message_read_basic(self.0.as_ptr(),kind as c_char,(&mut value as *mut T).cast())},"read basic")?;if count!=1{return Err("missing typed systemd value".into());}Ok(value)}
 fn text(&self,kind:u8)->Probe<String>{let pointer:*const c_char=self.basic(kind)?;if pointer.is_null(){return Err("null systemd string".into());}let bytes=unsafe{CStr::from_ptr(pointer)}.to_bytes();if bytes.len()>4096{return Err("systemd string exceeds bound".into());}String::from_utf8(bytes.to_vec()).map_err(|_|"non-UTF8 systemd string".into())}
 fn enter(&self,kind:u8,signature:&CStr)->Probe<c_int>{call(unsafe{sd_bus_message_enter_container(self.0.as_ptr(),kind as c_char,signature.as_ptr())},"enter systemd container")}
 fn leave(&self)->Probe<()>{call(unsafe{sd_bus_message_exit_container(self.0.as_ptr())},"exit systemd container").map(|_|())}
 fn bytes16(&self)->Probe<[u8;16]>{let mut data=ptr::null();let mut size=0;let count=call(unsafe{sd_bus_message_read_array(self.0.as_ptr(),b'y' as c_char,&mut data,&mut size)},"read invocation bytes")?;if count!=1||size!=16||data.is_null(){return Err("invalid systemd invocation ID".into());}let mut result=[0;16];result.copy_from_slice(unsafe{std::slice::from_raw_parts(data.cast(),16)});if result==[0;16]{return Err("zero systemd invocation ID".into());}Ok(result)}
}
impl Drop for Message{fn drop(&mut self){unsafe{sd_bus_message_unref(self.0.as_ptr());}}}
// libsystemd owns its connection FD; it is never transferred to a client.
pub(super) struct Bus{raw:NonNull<SdBus>,private:bool,public_destination:CString,manager_identity:Option<OwnedFd>,endpoint:Option<supervisor::native_activation::PublicBusEndpoint>}
impl Bus{
 fn open(private:bool,until:u64)->Probe<Self>{let mut raw=ptr::null_mut();let status=unsafe{sd_bus_new(&mut raw)};let Some(raw)=NonNull::new(raw)else{call(status,"new systemd bus")?;return Err("null systemd bus".into());};let mut bus=Self{raw,private,public_destination:DEST.to_owned(),manager_identity:None,endpoint:None};call(status,"new systemd bus")?;if !private{bus.endpoint=Some(supervisor::native_activation::PublicBusEndpoint::capture()?);}
  let address=if private{c"unix:path=/run/systemd/private"}else{c"unix:path=/run/dbus/system_bus_socket"};
  bus.bound(until)?;call(unsafe{sd_bus_set_address(bus.raw.as_ptr(),address.as_ptr())},"fixed systemd address")?;call(unsafe{sd_bus_set_bus_client(bus.raw.as_ptr(),if private{0}else{1})},"systemd bus mode")?;call(unsafe{sd_bus_start(bus.raw.as_ptr())},"connect systemd bus")?;
  if private{let fd=call(unsafe{sd_bus_get_fd(bus.raw.as_ptr())},"systemd bus FD")?;let peer=kernel_peer_credentials(fd).map_err(|e|format!("{e:?}"))?;if peer!=(Credentials{pid:1,uid:0,gid:0}){return Err("private bus is not root PID1".into());}}
  else{let fd=call(unsafe{sd_bus_get_fd(bus.raw.as_ptr())},"public bus FD")?;bus.endpoint.as_mut().ok_or("missing protected public endpoint")?.authenticate(fd)?;let owner=bus.daemon_text(c"GetNameOwner",c"org.freedesktop.systemd1",until)?;let name=CString::new(owner).map_err(|_|"systemd owner NUL")?;let pid=bus.daemon_number(c"GetConnectionUnixProcessID",&name,until)?;let uid=bus.daemon_number(c"GetConnectionUnixUser",&name,until)?;if pid!=1||uid!=0{return Err("public bus systemd owner is not root PID1".into());}bus.public_destination=name;}
  let manager=pidfd_for(1).map_err(|e|format!("PID1 capability: {e:?}"))?;validate_pidfd(&manager,1,&mut NativeReceipt::default()).map_err(|e|format!("PID1 capability: {e:?}"))?;bus.manager_identity=Some(manager);Ok(bus)
 }
 pub(super) fn root(until:u64)->Probe<Self>{Self::open(true,until)}
 pub(super) fn public(until:u64)->Probe<Self>{Self::open(false,until)}
 fn destination(&self)->*const c_char{if self.private{ptr::null()}else{self.public_destination.as_ptr()}}
 fn bound(&self,until:u64)->Probe<()>{if let Some(endpoint)=&self.endpoint{endpoint.revalidate()?;}let now=supervisor::boottime_ns()?;let left=until.checked_sub(now).filter(|n|*n>0).ok_or("manager operation deadline expired")?;let usec=left.div_ceil(1000).min(500_000);call(unsafe{sd_bus_set_method_call_timeout(self.raw.as_ptr(),usec)},"bound systemd operation")?;Ok(())}
 fn daemon(&self,method:&CStr,name:&CStr,until:u64)->Probe<Message>{self.bound(until)?;let mut reply=ptr::null_mut();let status=unsafe{sd_bus_call_method(self.raw.as_ptr(),c"org.freedesktop.DBus".as_ptr(),c"/org/freedesktop/DBus".as_ptr(),c"org.freedesktop.DBus".as_ptr(),method.as_ptr(),ptr::null_mut(),&mut reply,c"s".as_ptr(),name.as_ptr())};owned_reply(status,reply,"authenticate public systemd owner")}
 fn daemon_text(&self,method:&CStr,name:&CStr,until:u64)->Probe<String>{self.daemon(method,name,until)?.text(b's')}
 fn daemon_number(&self,method:&CStr,name:&CStr,until:u64)->Probe<u32>{self.daemon(method,name,until)?.basic(b'u')}
 fn property(&self,path:&CStr,interface:&CStr,name:&CStr,signature:&CStr,until:u64)->Probe<Message>{self.bound(until)?;let mut reply=ptr::null_mut();let status=unsafe{sd_bus_get_property(self.raw.as_ptr(),self.destination(),path.as_ptr(),interface.as_ptr(),name.as_ptr(),ptr::null_mut(),&mut reply,signature.as_ptr())};owned_reply(status,reply,"get authoritative systemd property")}
 fn text(&self,path:&CStr,interface:&CStr,name:&CStr,until:u64)->Probe<String>{self.property(path,interface,name,c"s",until)?.text(b's')}
 fn state(&self,path:&CStr,until:u64)->Probe<(String,String)>{
  self.bound(until)?;let mut raw=ptr::null_mut();let status=unsafe{sd_bus_call_method(self.raw.as_ptr(),self.destination(),path.as_ptr(),c"org.freedesktop.DBus.Properties".as_ptr(),c"GetAll".as_ptr(),ptr::null_mut(),&mut raw,c"s".as_ptr(),UNIT.as_ptr())};let reply=owned_reply(status,raw,"get atomic unit state")?;
  if reply.enter(b'a',c"{sv}")?!=1{return Err("unit state dictionary missing".into());}let mut active=None;let mut sub=None;let mut count=0;
  while reply.enter(b'e',c"sv")?==1{count+=1;if count>256{return Err("unit state dictionary exceeds bound".into());}let key=reply.text(b's')?;
   if matches!(key.as_str(),"ActiveState"|"SubState"){if reply.enter(b'v',c"s")?!=1{return Err("unit state variant missing".into());}let value=reply.text(b's')?;reply.leave()?;let target=if key=="ActiveState"{&mut active}else{&mut sub};if target.replace(value).is_some(){return Err("duplicate unit state".into());}}
   else if call(unsafe{sd_bus_message_skip(reply.0.as_ptr(),c"v".as_ptr())},"skip other unit property")?!=1{return Err("unit state variant cannot be skipped".into());}reply.leave()?;
  }reply.leave()?;Ok((active.ok_or("ActiveState missing")?,sub.ok_or("SubState missing")?))
 }
 fn strings(&self,path:&CStr,interface:&CStr,name:&CStr,until:u64)->Probe<Vec<String>>{
  let reply=self.property(path,interface,name,c"as",until)?;if reply.enter(b'a',c"s")?!=1{return Err("missing manager string array".into());}let mut values=Vec::new();loop{let mut pointer:*const c_char=ptr::null();let count=call(unsafe{sd_bus_message_read_basic(reply.0.as_ptr(),b's' as c_char,(&mut pointer as *mut *const c_char).cast())},"manager array item")?;if count==0{break;}if pointer.is_null()||values.len()==64{return Err("manager array bound".into());}let bytes=unsafe{CStr::from_ptr(pointer)}.to_bytes();if bytes.len()>4096{return Err("manager array string bound".into());}values.push(String::from_utf8(bytes.to_vec()).map_err(|_|"manager array UTF8")?);}reply.leave()?;Ok(values)
 }
 fn empty_commands(&self,path:&CStr,name:&CStr,until:u64)->Probe<()>{let reply=self.property(path,SERVICE,name,c"a(sasbttttuii)",until)?;if reply.enter(b'a',c"(sasbttttuii)")?!=1||reply.enter(b'r',c"sasbttttuii")?!=0{return Err("unexpected service bootstrap/teardown command".into());}reply.leave()}
 fn socket_policy(&self,service:&CStr,until:u64)->Probe<()>{
  if self.strings(service,UNIT,c"TriggeredBy",until)?!=["process-tree-guard.socket"]||!self.strings(service,UNIT,c"BindsTo",until)?.iter().any(|v|v=="process-tree-guard.socket"){return Err("service not bound to protected activation socket".into());}
  let socket=self.unit_for_name(c"process-tree-guard.socket",until)?;
  if self.text(&socket,UNIT,c"FragmentPath",until)?!=SOCKET_FILE||!self.strings(&socket,UNIT,c"DropInPaths",until)?.is_empty()||self.text(&socket,UNIT,c"SourcePath",until)?!=""||self.number::<i32>(&socket,UNIT,c"Transient",c"b",b'b',until)?!=0{return Err("activation socket file provenance differs".into());}
  for name in [c"Accept",c"PassCredentials"]{if self.number::<i32>(&socket,c"org.freedesktop.systemd1.Socket",name,c"b",b'b',until)?!=1{return Err("activation socket credential/accept policy differs".into());}}
  let reply=self.property(&socket,c"org.freedesktop.systemd1.Socket",c"Listen",c"a(ss)",until)?;
  if reply.enter(b'a',c"(ss)")?!=1||reply.enter(b'r',c"ss")?!=1||reply.text(b's')?!="SequentialPacket"||reply.text(b's')?!=ACTIVATE{return Err("activation socket listen address differs".into());}reply.leave()?;if reply.enter(b'r',c"ss")?!=0{return Err("extra activation listen endpoint".into());}reply.leave()?;Ok(())
 }
 fn number<T:Default>(&self,path:&CStr,interface:&CStr,name:&CStr,signature:&CStr,kind:u8,until:u64)->Probe<T>{self.property(path,interface,name,signature,until)?.basic(kind)}
 fn unit_for_pid(&self,pid:u32,until:u64)->Probe<CString>{self.bound(until)?;let mut reply=ptr::null_mut();let status=unsafe{sd_bus_call_method(self.raw.as_ptr(),self.destination(),ROOT.as_ptr(),MANAGER.as_ptr(),c"GetUnitByPID".as_ptr(),ptr::null_mut(),&mut reply,c"u".as_ptr(),pid)};CString::new(owned_reply(status,reply,"unit for actual PID")?.text(b'o')?).map_err(|_|"unit object NUL".into())}
 fn unit_for_name(&self,name:&CStr,until:u64)->Probe<CString>{self.bound(until)?;let mut reply=ptr::null_mut();let status=unsafe{sd_bus_call_method(self.raw.as_ptr(),self.destination(),ROOT.as_ptr(),MANAGER.as_ptr(),c"GetUnit".as_ptr(),ptr::null_mut(),&mut reply,c"s".as_ptr(),name.as_ptr())};CString::new(owned_reply(status,reply,"get referenced unit")?.text(b'o')?).map_err(|_|"unit object NUL".into())}
 fn ref_operation(&self,name:&CStr,member:&CStr,until:u64)->Probe<()>{self.bound(until)?;let mut error=BusError::new();let mut reply=ptr::null_mut();let status=unsafe{sd_bus_call_method(self.raw.as_ptr(),self.destination(),ROOT.as_ptr(),MANAGER.as_ptr(),member.as_ptr(),&mut error.0,&mut reply,c"s".as_ptr(),name.as_ptr())};let owned=NonNull::new(reply).map(Message);let operation=reference_operation(member,name,&error.0);if status<0{drop(owned);return Err(format!("{operation}: libsystemd error {status}"));}drop(owned.ok_or_else(||format!("{operation}: missing systemd reply"))?);Ok(())}
 fn invocation(&self,path:&CStr,until:u64)->Probe<[u8;16]>{self.property(path,UNIT,c"InvocationID",c"ay",until)?.bytes16()}
 fn exec(&self,path:&CStr,until:u64)->Probe<(u32,i32,i32)>{let reply=self.property(path,SERVICE,c"ExecStart",c"a(sasbttttuii)",until)?;if reply.enter(b'a',c"(sasbttttuii)")?!=1||reply.enter(b'r',c"sasbttttuii")?!=1{return Err("missing exact ExecStart".into());}if reply.text(b's')?!=HELPER{return Err("unit ExecStart path differs".into());}if reply.enter(b'a',c"s")?!=1{return Err("missing ExecStart argv".into());}let a=reply.text(b's')?;let b=reply.text(b's')?;let mut extra:*const c_char=ptr::null();if a!=HELPER||b!="--service"||call(unsafe{sd_bus_message_read_basic(reply.0.as_ptr(),b's' as c_char,(&mut extra as *mut *const c_char).cast())},"ExecStart tail")?!=0{return Err("unit ExecStart argv differs".into());}reply.leave()?;let ignore:i32=reply.basic(b'b')?;for _ in 0..4{let _:u64=reply.basic(b't')?;}let pid=reply.basic(b'u')?;let code=reply.basic(b'i')?;let status=reply.basic(b'i')?;reply.leave()?;if ignore!=0||reply.enter(b'r',c"sasbttttuii")?!=0{return Err("unit has extra/ignored ExecStart".into());}reply.leave()?;Ok((pid,code,status))}
}
impl Drop for Bus{fn drop(&mut self){unsafe{sd_bus_unref(self.raw.as_ptr());}}}

const MAX_RUNTIME_USEC:u64=86_430_000_000;
#[derive(Clone,Copy,PartialEq,Eq)]enum UnitPhase{Starting,Running}
fn phase_allows(phase:UnitPhase,active:&str,sub:&str)->bool{match phase{UnitPhase::Starting=>active=="activating"&&sub=="start",UnitPhase::Running=>active=="active"&&sub=="running"}}
fn notify_policy(bus:&Bus,path:&CStr,until:u64)->Probe<()>{
 for(name,wanted)in [(c"Type","notify"),(c"NotifyAccess","main"),(c"ExitType","main"),(c"Restart","no"),(c"KillMode","control-group"),(c"TimeoutStartFailureMode","kill"),(c"TimeoutStopFailureMode","kill")]{if bus.text(path,SERVICE,name,until)?!=wanted{return Err("fixed notify service policy differs".into());}}
 for(name,wanted)in [(c"SendSIGKILL",1),(c"RemainAfterExit",0)]{if bus.number::<i32>(path,SERVICE,name,c"b",b'b',until)?!=wanted{return Err("notify exit/kill policy differs".into());}}
 for name in [c"KillSignal",c"FinalKillSignal"]{if bus.number::<i32>(path,SERVICE,name,c"i",b'i',until)?!=9{return Err("notify recursive kill signal differs".into());}}
 for(name,wanted)in [(c"RuntimeMaxUSec",MAX_RUNTIME_USEC),(c"TimeoutStartUSec",20_000_000),(c"TimeoutStopUSec",5_000_000),(c"RuntimeRandomizedExtraUSec",0),(c"WatchdogUSec",0)]{if bus.number::<u64>(path,SERVICE,name,c"t",b't',until)?!=wanted{return Err("fixed notify finite ceiling differs".into());}}
 for name in [c"JobTimeoutUSec",c"JobRunningTimeoutUSec"]{if bus.number::<u64>(path,UNIT,name,c"t",b't',until)?!=20_000_000{return Err("fixed activation job bound differs".into());}}Ok(())
}
// Fields cannot be supplied by a request or another module. This is created
// only by authenticated manager calls plus actual protected/kernel checks.
pub(super) struct CheckedUnit{
 bus:Bus,path:CString,name:CString,invocation:[u8;16],main_pid:u32,control_group:String,
 phase:UnitPhase,
 protected:supervisor::native_activation::EntryKernelAuthority,
}
impl CheckedUnit{
 pub(super) fn capture(until:u64)->Probe<Self>{let protected=supervisor::native_activation::capture_entry_kernel_authority()?;let bus=Bus::root(until)?;let path=bus.unit_for_pid(std::process::id(),until)?;let name=CString::new(bus.text(&path,UNIT,c"Id",until)?).map_err(|_|"unit ID NUL")?;let id=name.to_str().map_err(|_|"unit ID UTF8")?;if !id.starts_with(PREFIX)||!id.ends_with(".service")||id.len()>255||!id.bytes().all(|b|b.is_ascii_alphanumeric()||b"@._:-\\".contains(&b)){return Err("not the protected one-shot service template".into());}
  let invocation=bus.invocation(&path,until)?;let main_pid:u32=bus.number(&path,SERVICE,c"MainPID",c"u",b'u',until)?;if main_pid!=std::process::id(){return Err("manager MainPID differs".into());}let (exec_pid,_,_)=bus.exec(&path,until)?;if exec_pid!=main_pid{return Err("manager ExecStart PID differs".into());}
  let control_group=bus.text(&path,SERVICE,c"ControlGroup",until)?;if !control_group.starts_with('/')||control_group.len()>4096{return Err("invalid manager control group".into());}
  let checked=Self{bus,path,name,invocation,main_pid,control_group,phase:UnitPhase::Starting,protected};checked.validate(until)?;Ok(checked)
 }
 pub(super) fn validate(&self,until:u64)->Probe<()>{self.validate_policy(until)?;let(active,sub)=self.bus.state(&self.path,until)?;if !phase_allows(self.phase,&active,&sub){return Err("service phase differs".into());}Ok(())}
 fn validate_policy(&self,until:u64)->Probe<()>{self.protected.revalidate()?;
  if self.bus.text(&self.path,UNIT,c"FragmentPath",until)?!=TEMPLATE_FILE||self.bus.text(&self.path,UNIT,c"SourcePath",until)?!=""||!self.bus.strings(&self.path,UNIT,c"DropInPaths",until)?.is_empty()||self.bus.number::<i32>(&self.path,UNIT,c"Transient",c"b",b'b',until)?!=0{return Err("service manager file provenance differs from held template".into());}
  if self.bus.text(&self.path,SERVICE,c"ControlGroup",until)?!=self.control_group||self.bus.exec(&self.path,until)?.0!=self.main_pid{return Err("service cgroup/executable identity changed".into());}
  for name in [c"ExecStartPre",c"ExecStartPost",c"ExecCondition",c"ExecStop",c"ExecStopPost",c"ExecReload"]{self.bus.empty_commands(&self.path,name,until)?;}
  for name in [c"RootDirectory",c"RootImage"]{if !self.bus.text(&self.path,SERVICE,name,until)?.is_empty(){return Err("service root filesystem override refused".into());}}
  for name in [c"Environment",c"PassEnvironment"]{if !self.bus.strings(&self.path,SERVICE,name,until)?.is_empty(){return Err("service environment override refused".into());}}
  if self.bus.number::<i32>(&self.path,SERVICE,c"NoNewPrivileges",c"b",b'b',until)?!=1{return Err("service NoNewPrivileges differs".into());}
  let runtime_directory=format!("process-tree-guard/{}",self.name.to_str().map_err(|_|"unit runtime directory UTF8")?);
  if self.bus.strings(&self.path,SERVICE,c"RuntimeDirectory",until)?!=[runtime_directory]||self.bus.number::<u32>(&self.path,SERVICE,c"RuntimeDirectoryMode",c"u",b'u',until)?!=0o755||self.bus.text(&self.path,SERVICE,c"RuntimeDirectoryPreserve",until)?!="no"{return Err("per-service PID1 runtime-directory cleanup policy differs".into());}
  self.bus.socket_policy(&self.path,until)?;
if self.bus.invocation(&self.path,until)?!=self.invocation||self.bus.number::<u32>(&self.path,SERVICE,c"MainPID",c"u",b'u',until)?!=self.main_pid{return Err("manager invocation identity changed".into());}
  notify_policy(&self.bus,&self.path,until)?;
  if self.bus.number::<i32>(&self.path,SERVICE,c"Delegate",c"b",b'b',until)?!=1||self.bus.text(&self.path,SERVICE,c"DelegateSubgroup",until)?!="supervisor"{return Err("service protected subgroup policy differs".into());}Ok(())
 }
 pub(super) fn enter_running(&mut self,notification:&mut entry::PendingNotify,original:&invocation::VerifiedInvocation,proof:&entry::OriginalDeadlineProof,until:u64)->Probe<()>{
  if self.phase!=UnitPhase::Starting{return Err("service activation handoff reused".into());}let until=until.min(proof.until());
  self.validate(until)?;original.check_live().map_err(|e|format!("{e:?}"))?;original.check_time().map_err(|e|format!("{e:?}"))?;proof.revalidate(original.metadata())?;
  notification.ready(until)?;
  loop{
   self.validate_policy(until)?;original.check_live().map_err(|e|format!("{e:?}"))?;original.check_time().map_err(|e|format!("{e:?}"))?;proof.revalidate(original.metadata())?;
   let(active,sub)=self.bus.state(&self.path,until)?;
   if phase_allows(UnitPhase::Running,&active,&sub){self.validate_policy(until)?;original.check_live().map_err(|e|format!("{e:?}"))?;original.check_time().map_err(|e|format!("{e:?}"))?;proof.revalidate(original.metadata())?;let(a,s)=self.bus.state(&self.path,until)?;if !phase_allows(UnitPhase::Running,&a,&s){return Err("running acknowledgment changed".into());}notification.consumed()?;self.phase=UnitPhase::Running;return Ok(());}
   if !phase_allows(UnitPhase::Starting,&active,&sub){return Err("PID1 READY handoff entered an invalid phase".into());}
   if supervisor::boottime_ns()?>=until{return Err("original READY handoff deadline expired".into());}std::thread::sleep(std::time::Duration::from_millis(1));
  }
 }
 pub(super) fn require_running(&self)->Probe<()>{if self.phase==UnitPhase::Running{Ok(())}else{Err("service has no completed runtime handoff".into())}}

 pub(super) fn protected_current_elf(&self)->Probe<supervisor::native_activation::ProtectedElfObservation>{self.protected.protected_current_elf()}
 pub(super) fn group(&self)->&str{&self.control_group}
 pub(super) fn name(&self)->&CStr{&self.name}
 pub(super) fn invocation(&self)->[u8;16]{self.invocation}
 pub(super) fn main_pid(&self)->u32{self.main_pid}
}

// Ordinary client owns this public-bus reference under an explicit administrator
// ref-only authorization; property read access alone does not grant RefUnit. No root bus
// descriptor or command capability crosses the privilege boundary.
pub(super) enum FinalStatus{Verified,NeedsRetirement}
pub(super) struct UnitReference{bus:Bus,path:CString,name:CString,invocation:[u8;16],pid:u32,group:String,referenced:bool}
impl UnitReference{
 pub(super) fn acquire(name:&CStr,invocation:[u8;16],pid:u32,group:String,until:u64)->Probe<Self>{let bus=Bus::public(until)?;bus.ref_operation(name,c"RefUnit",until)?;let path=bus.unit_for_name(name,until)?;let result=Self{bus,path,name:name.to_owned(),invocation,pid,group,referenced:true};result.live(until)?;Ok(result)}
 pub(super) fn live(&self,until:u64)->Probe<()>{if self.bus.invocation(&self.path,until)?!=self.invocation||self.bus.number::<u32>(&self.path,SERVICE,c"MainPID",c"u",b'u',until)?!=self.pid||self.bus.text(&self.path,SERVICE,c"ControlGroup",until)?!=self.group{return Err("root unit reference identity differs".into());}let (pid,_,_)=self.bus.exec(&self.path,until)?;if pid!=self.pid{return Err("root ExecStart identity differs".into());}notify_policy(&self.bus,&self.path,until)?;let(active,sub)=self.bus.state(&self.path,until)?;if !phase_allows(UnitPhase::Running,&active,&sub){return Err("referenced root service is not running".into());}Ok(())}
 pub(super) fn terminal_code(&self,until:u64)->Probe<i32>{
  if self.bus.invocation(&self.path,until)?!=self.invocation||self.bus.number::<u32>(&self.path,SERVICE,c"ExecMainPID",c"u",b'u',until)?!=self.pid||self.bus.number::<i32>(&self.path,SERVICE,c"ExecMainCode",c"i",b'i',until)?!=1{return Err("unframed terminal incarnation/status is unverified".into());}
  let status:i32=self.bus.number(&self.path,SERVICE,c"ExecMainStatus",c"i",b'i',until)?;if !(0..=255).contains(&status){return Err("unframed service status out of range".into());}Ok(status)
 }
 pub(super) fn final_status(&self,reported:i32,retired:Option<&supervisor::native_activation::RetiredUnit>,until:u64)->Probe<FinalStatus>{
  let group=self.bus.text(&self.path,SERVICE,c"ControlGroup",until)?;
  if self.bus.invocation(&self.path,until)?!=self.invocation||!(group==self.group||group.is_empty()){return Err("terminal unit incarnation differs".into());}
  let pid:u32=self.bus.number(&self.path,SERVICE,c"ExecMainPID",c"u",b'u',until)?;let code:i32=self.bus.number(&self.path,SERVICE,c"ExecMainCode",c"i",b'i',until)?;let status:i32=self.bus.number(&self.path,SERVICE,c"ExecMainStatus",c"i",b'i',until)?;let result=self.bus.text(&self.path,SERVICE,c"Result",until)?;
  if pid!=self.pid||code!=1||status!=reported||!matches!(result.as_str(),"success"|"exit-code"){return Err(format!("root service terminal mismatch pid={pid} code={code} status={status} result={result}"));}
  if group.is_empty(){return match retired{Some(proof)if proof.matches_group(&self.group)=>Ok(FinalStatus::Verified),None=>Ok(FinalStatus::NeedsRetirement),_=>Err("retirement receipt belongs to another unit".into())};}Ok(FinalStatus::Verified)
 }
 pub(super) fn release(&mut self,until:u64)->Probe<()>{if self.referenced{self.bus.ref_operation(&self.name,c"UnrefUnit",until)?;self.referenced=false;}Ok(())}
}

#[cfg(test)]pub(super) fn notify_handoff_regression(){
 let pointer=std::mem::size_of::<*const c_char>();let alignment=std::mem::align_of::<*const c_char>();
 assert_eq!(std::mem::offset_of!(SdBusError,name),0);assert_eq!(std::mem::offset_of!(SdBusError,message),pointer);assert_eq!(std::mem::offset_of!(SdBusError,_need_free),2*pointer);
 assert_eq!(std::mem::align_of::<SdBusError>(),alignment);assert_eq!(std::mem::size_of::<SdBusError>(),(2*pointer+std::mem::size_of::<c_int>()).div_ceil(alignment)*alignment);
 let raw=SdBusError{name:c"org.freedesktop.DBus.Error.AccessDenied".as_ptr(),message:c"denied\n\"\\".as_ptr(),_need_free:0};
 let diagnostic=reference_operation(c"RefUnit",c"process-tree-guard@0-2-1000.service",&raw);assert!(diagnostic.contains("member=\"RefUnit\""));assert!(diagnostic.contains("error_name=\"org.freedesktop.DBus.Error.AccessDenied\""));assert!(diagnostic.contains("error_message=\"denied\\x0a\\x22\\x5c\""));assert!(!diagnostic.contains('\n'));
 assert_eq!(bounded_error_field(ptr::null(),256),"<null>");assert_eq!(bounded_error_field(c"".as_ptr(),1024),"\"\"");
 let long=[b'x';1024];let truncated=bounded_error_field(long.as_ptr().cast(),1024);assert_eq!(truncated.len(),1024+2+"<truncated>".len());assert!(truncated.ends_with("<truncated>"));
 let invalid=[0xffu8,0];assert_eq!(bounded_error_field(invalid.as_ptr().cast(),1024),"\"\\xff\"");assert!(reference_operation(c"UnrefUnit",c"unit",&raw).contains("member=\"UnrefUnit\""));
 eprintln!("unit_reference_error_layout_bounded_format_pure_verified");

 assert_eq!(MAX_RUNTIME_USEC,((crate::deadline::MAX_SECONDS as u64)+30)*1_000_000);
 for(phase,active,sub,expected)in [(UnitPhase::Starting,"activating","start",true),(UnitPhase::Running,"active","running",true),(UnitPhase::Starting,"active","running",false),(UnitPhase::Running,"activating","start",false),(UnitPhase::Running,"active","exited",false),(UnitPhase::Running,"failed","failed",false),(UnitPhase::Starting,"deactivating","stop-sigkill",false),(UnitPhase::Running,"active","reload",false)]{assert_eq!(phase_allows(phase,active,sub),expected);}
 eprintln!("notify_phase_regression exact_starting_running_ceiling_verified");
}
