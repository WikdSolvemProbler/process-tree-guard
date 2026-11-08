// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
//! Original in-process caller; no sudo, daemon, relay or library NNP mutation.
use super::*;
use std::ffi::{OsString,CString,c_long,c_char};
use std::os::fd::AsFd;
use std::os::unix::ffi::{OsStrExt,OsStringExt};
use std::os::unix::fs::OpenOptionsExt;
const OBSERVE_GRACE:u64=1_000_000_000;
unsafe extern "C"{fn prctl(option:c_int,...)->c_int;fn access(path:*const c_char,mode:c_int)->c_int;}
#[must_use="inspect completion versus retained uncertain service capabilities"]
pub(crate) struct ClientOutcome{
 code:i32,error:Option<String>,original:Option<OriginalInvocation>,
 activation:Option<OwnedFd>,private:Option<OwnedFd>,service:Option<OwnedFd>,events:Option<fs::File>,
 unit:Option<manager::UnitReference>,completion:Option<supervisor::native_activation::ClientCompletion>,private_runtime:Option<supervisor::native_activation::PrivateRuntimeDirectory>,protected:Vec<OwnedFd>,hello:Option<entry::Hello>,
 raw_hello:Vec<u8>,raw_result:Vec<u8>,result:Option<entry::DecodedOutcome>,exit_timer:Option<std::rc::Rc<entry::ExitTimer>>,presentation_attempted:bool,completed:bool,
}
impl ClientOutcome{
 fn empty()->Self{Self{code:125,error:None,original:None,activation:None,private:None,service:None,events:None,unit:None,completion:None,private_runtime:None,protected:Vec::new(),hello:None,raw_hello:Vec::new(),raw_result:Vec::new(),result:None,exit_timer:None,presentation_attempted:false,completed:false}}
 pub(crate) fn exit_code(&self)->i32{self.code}
 pub(crate) fn error(&self)->Option<&str>{self.error.as_deref()}
 pub(crate) fn complete(&self)->bool{self.completed}
 /// Conservative owning quarantine: a live library host may retain a service
 /// until the original cutoff; this is NOT teardown-on-error parity.
 pub(crate) fn may_have_active_service(&self)->bool{self.presentation_attempted&&!self.completed}
 pub(crate) fn wire_evidence(&self)->(&[u8],&[u8]){(&self.raw_hello,&self.raw_result)}
}
fn error<T>(value:super::Result<T>)->std::result::Result<T,String>{value.map_err(|e|format!("{e:?}"))}
fn caller_io()->std::result::Result<CallerIo,String>{
 let mut fds=Vec::new();for number in 0..3{let duplicate=error(os_result(unsafe{fcntl(number,1030,3 as c_int)} as isize))? as RawFd;let fd=unsafe{OwnedFd::from_raw_fd(duplicate)};error(io_flags(fd.as_raw_fd(),number as usize))?;fds.push(fd);}
 let stdio:[OwnedFd;3]=fds.try_into().map_err(|_|"stdio role count")?;
 let cwd=fs::OpenOptions::new().read(true).custom_flags(O_PATH|O_DIRECTORY|SOCK_CLOEXEC).open(".").map_err(|e|e.to_string())?;
 Ok(CallerIo{stdio,cwd:cwd.into()})
}
fn command_absolute(command:&[OsString])->std::result::Result<Vec<Vec<u8>>,String>{
 let first=command.first().ok_or("a command is required")?;let bytes=first.as_bytes();if bytes.is_empty()||bytes.contains(&0){return Err("invalid program".into());}
 let candidate=std::path::PathBuf::from(first);let absolute=if candidate.is_absolute(){candidate}
 else if bytes.contains(&b'/'){std::env::current_dir().map_err(|e|e.to_string())?.join(candidate)}else{
  let path=std::env::var_os("PATH").ok_or("caller PATH is missing")?;let mut chosen=None;
  for directory in std::env::split_paths(&path){let directory=if directory.is_absolute(){directory}else{std::env::current_dir().map_err(|e|e.to_string())?.join(directory)};let file=directory.join(first);if file.is_file(){let name=CString::new(file.as_os_str().as_bytes()).map_err(|_|"program path NUL")?;if unsafe{access(name.as_ptr(),1)}==0{chosen=Some(file);break;}}}
  chosen.ok_or("program was not found in caller PATH")?
 };
 let mut argv=vec![absolute.into_os_string().into_vec()];argv.extend(command[1..].iter().map(|v|v.as_bytes().to_vec()));Ok(argv)
}
fn environment()->Vec<Vec<u8>>{std::env::vars_os().map(|(key,value)|{let mut bytes=key.into_vec();bytes.push(b'=');bytes.extend(value.into_vec());bytes}).collect()}
fn service_gone(fd:&OwnedFd)->std::result::Result<bool,String>{let mut entry=PollFd{fd:fd.as_raw_fd(),events:POLLIN,revents:0};let count=error(os_result(unsafe{poll(&mut entry,1,0)} as isize))?;if count==0&&entry.revents==0{return Ok(false);}if count==1&&entry.revents&(POLLIN|0x10)!=0&&entry.revents&!(POLLIN|0x10)==0{return Ok(true);}Err("service pidfd poll unverified".into())}
fn run_captured(original:OriginalInvocation,command:&[OsString],standalone:bool)->ClientOutcome{
 let mut outcome=ClientOutcome::empty();outcome.original=Some(original);
 let result=(||->std::result::Result<i32,String>{
  let absolute=outcome.original.as_ref().expect("captured original").metadata().deadline_ns;
  if standalone{let until=absolute.checked_add(entry::GRACE+OBSERVE_GRACE).ok_or("CLI exit deadline overflow")?;match entry::cli_timer(until){Ok(owner)=>outcome.exit_timer=Some(owner),Err((owner,error))=>{outcome.exit_timer=Some(owner);return Err(error);}}}
  supervisor::native_activation::require_initial_client_mapping()?;
  outcome.protected=supervisor::native_activation::client_installation()?;
  if standalone{if unsafe{prctl(38,1 as c_long,0 as c_long,0 as c_long,0 as c_long)}!=0{return Err(format!("CLI NNP set: {}",io::Error::last_os_error()));}}
  let nnp=unsafe{prctl(39,0 as c_long,0 as c_long,0 as c_long,0 as c_long)};
  if nnp!=1{return Err("Linux library requires existing NoNewPrivs=1; caller policy was not changed".into());}
  let original=outcome.original.as_ref().expect("captured original owner");original.revalidate().map_err(|e|format!("{e:?}"))?;
  let end=original.metadata().deadline_ns.checked_add(entry::GRACE).ok_or("client cleanup deadline overflow")?;
  let observation_end=end.checked_add(OBSERVE_GRACE).ok_or("client observation deadline overflow")?;
  let argv=command_absolute(command)?;let env=environment();let io=caller_io()?;
  let args:Vec<_>=argv.iter().map(Vec::as_slice).collect();let env:Vec<_>=env.iter().map(Vec::as_slice).collect();
  // Resolve/validate all context before activation, under the original cutoff.
  request::encode(original.metadata(),&args,&env).map_err(|e|format!("{e:?}"))?;
  original.revalidate().map_err(|e|format!("{e:?}"))?;
  outcome.activation=Some(entry::connect_root(manager::ACTIVATE,original.metadata().deadline_ns)?);
  let activation=outcome.activation.as_ref().expect("activation owned");
  error(send_request(activation,original,&args,&env,io.stdio.each_ref().map(|fd|fd.as_fd()),io.cwd.as_fd()))?;
  let packet=entry::packet(activation.as_raw_fd(),end,8192)?;outcome.raw_hello=packet.bytes.clone();
  if packet.bytes.starts_with(b"PTGOUTC1"){if packet.sender.pid<=1||packet.sender.uid!=0||packet.sender.gid!=0||!packet.rights.is_empty(){return Err("bootstrap refusal sender/rights invalid".into());}let refusal=entry::DecodedOutcome::decode(&packet.bytes,[0;16],0)?;if refusal.code!=125||refusal.settled||refusal.error.is_empty(){return Err("bootstrap refusal framing invalid".into());}let error=format!("root bootstrap: {}",refusal.error);outcome.result=Some(refusal);return Err(error);}
  let hello=entry::Hello::decode(&packet.bytes)?;
  if packet.sender!=(Credentials{pid:hello.pid as c_int,uid:0,gid:0})||packet.rights.len()!=2||hello.cutoff!=end{return Err("root hello sender/capability/cutoff mismatch".into());}
  let [service,events]:[OwnedFd;2]=packet.rights.try_into().map_err(|_|"root hello rights")?;
  // Adopt both capabilities before all later refusals; never reopen service PID.
  outcome.service=Some(service);outcome.events=Some(fs::File::from(events));outcome.hello=Some(hello.clone());
  validate_pidfd(outcome.service.as_ref().expect("owned service"),hello.pid as c_int,&mut NativeReceipt::default()).map_err(|e|format!("service capability: {e:?}"))?;
  let name=CString::new(hello.unit.clone()).map_err(|_|"unit name NUL")?;
  outcome.unit=Some(manager::UnitReference::acquire(&name,hello.invocation,hello.pid,hello.group.clone(),end)?);
  let events=outcome.events.as_ref().expect("owned events");let duplicate=error(os_result(unsafe{fcntl(events.as_raw_fd(),1030,3 as c_int)} as isize))? as RawFd;
  let duplicate=unsafe{OwnedFd::from_raw_fd(duplicate)};
  outcome.completion=Some(supervisor::native_activation::client_events(&duplicate,&hello.group)?);drop(duplicate);
  original.revalidate().map_err(|e|format!("{e:?}"))?;
  outcome.private_runtime=Some(supervisor::native_activation::PrivateRuntimeDirectory::capture(&hello.unit)?);
  if hello.socket!=outcome.private_runtime.as_ref().expect("held private directory").path()?||!hello.unit.starts_with("process-tree-guard@")||!hello.unit.ends_with(".service")||hello.unit.len()>255{return Err("root hello unit/private path binding differs".into());}
  outcome.private=Some(entry::connect_root(&hello.socket,original.metadata().deadline_ns)?);
  let private=outcome.private.as_ref().expect("private connection owned");let peer=kernel_peer_credentials(private.as_raw_fd()).map_err(|e|format!("{e:?}"))?;
  if peer!=(Credentials{pid:hello.pid as c_int,uid:0,gid:0}){return Err("private root peer differs from held service".into());}
  validate_pidfd(outcome.service.as_ref().expect("held service"),hello.pid as c_int,&mut NativeReceipt::default()).map_err(|e|format!("{e:?}"))?;
  outcome.presentation_attempted=true;
  error(send_request(private,original,&args,&env,io.stdio.each_ref().map(|fd|fd.as_fd()),io.cwd.as_fd()))?;
  let packet=entry::packet(private.as_raw_fd(),observation_end,8192)?;outcome.raw_result=packet.bytes;
  if !packet.rights.is_empty()||packet.sender!=peer{return Err("root result sender/capabilities differ".into());}
  outcome.result=Some(entry::DecodedOutcome::decode(&outcome.raw_result,hello.invocation,end)?);
  let decoded=outcome.result.as_ref().expect("owned root result");let code=decoded.code;let settled=decoded.settled;let root_error=decoded.error.clone();
  // The framed result is PROVISIONAL through actual process exit, unit empty,
  // and exact retained manager ExecMainPID/code/status/result publication.
  loop{
   if supervisor::boottime_ns()? >= observation_end{return Err("root service settlement observation expired".into());}
   if service_gone(outcome.service.as_ref().expect("held service"))?&&!outcome.completion.as_mut().expect("held completion authority").population_after_terminal(outcome.service.as_ref().expect("held service"),hello.pid)?{break;}
   let mut watched=PollFd{fd:outcome.service.as_ref().expect("held service").as_raw_fd(),events:POLLIN,revents:0};
   if unsafe{poll(&mut watched,1,10)}<0{return Err(format!("root settlement poll: {}",io::Error::last_os_error()));}
  }
  let status=outcome.unit.as_ref().expect("unit reference retained").final_status(code,outcome.completion.as_ref().expect("held completion authority").retired(),observation_end)?;
  if matches!(status,manager::FinalStatus::NeedsRetirement){
   // This is the authoritative final query's typed same-incarnation empty-path
   // case, not a preflight or a retry of arbitrary manager/read errors.
   if outcome.completion.as_mut().expect("held completion authority").population_after_terminal(outcome.service.as_ref().expect("held service"),hello.pid)?{return Err("terminal unit is still populated".into());}
   let proof=outcome.completion.as_ref().expect("held completion authority").retired().ok_or("manager pruned path without exact original retirement proof")?;
   if !matches!(outcome.unit.as_ref().expect("unit reference retained").final_status(code,Some(proof),observation_end)?,manager::FinalStatus::Verified){return Err("terminal retirement join remained unverified".into());}
  }
  outcome.unit.as_mut().expect("unit reference retained").release(observation_end)?;
  if !settled||!root_error.is_empty(){return Err(if root_error.is_empty(){"root native settlement is unverified".into()}else{root_error});}
  outcome.completed=true;Ok(code)
 })();
 match result{Ok(code)=>outcome.code=code,Err(error)=>{outcome.code=125;outcome.error=Some(if outcome.presentation_attempted{format!("{error}; nonsettled service quarantine retained; payload may continue until original cutoff") }else{error});}}
 outcome
}
/// Library host policy is read-only. Capture precedes every activation attempt.
pub(crate) fn run_library(command:&[OsString],seconds:f64,mode:OwnerMode)->ClientOutcome{
 match OriginalInvocation::capture(mode,seconds){Ok(original)=>run_captured(original,command,false),Err(error)=>{let mut result=ClientOutcome::empty();result.code=error.exit_code();result.error=Some(format!("original invocation: {error:?}"));result}}
}
/// Dedicated standalone CLI only: its own NNP may be set; no host is changed.
pub(crate) fn run_cli(arguments:Vec<OsString>)->ClientOutcome{
 let mut mode=OwnerMode::Manual;let mut index=0;let mut seconds=None;
 if arguments.first().is_some_and(|v|v=="auto"){mode=OwnerMode::Auto;index=1;}
 if arguments.get(index).is_some_and(|v|v=="--timeout"){
  seconds=arguments.get(index+1).and_then(|v|v.to_str()).and_then(|v|v.parse::<f64>().ok());index+=2;
 }else if mode==OwnerMode::Auto{
  let mut selected=false;
  for name in ["PROCESS_TREE_GUARD_AUTO_TIMEOUT","PROCESS_GUARD_AUTO_TIMEOUT","HARNESS_AUTO_TIMEOUT"]{if let Some(value)=std::env::var_os(name){selected=true;seconds=value.to_str().and_then(|v|v.parse().ok());break;}}
  if !selected{seconds=Some(7200.0);}

 }
 if arguments.get(index).is_some_and(|v|v=="--"){index+=1;}
 let Some(seconds)=seconds else{let mut result=ClientOutcome::empty();result.error=Some("usage: --timeout SECONDS -- PROGRAM ARGS, or auto [--timeout SECONDS] -- PROGRAM ARGS".into());return result;};
 match OriginalInvocation::capture(mode,seconds){Ok(original)=>run_captured(original,&arguments[index..],true),Err(error)=>{let mut result=ClientOutcome::empty();result.code=error.exit_code();result.error=Some(format!("original invocation: {error:?}"));result}}
}

/// Dedicated CLI only; retained owners are kept live until actual kernel exit.
pub(crate) fn exit(result:ClientOutcome)->!{entry::cli_exit(result.exit_code(),result.error(),result.exit_timer.as_deref())}
