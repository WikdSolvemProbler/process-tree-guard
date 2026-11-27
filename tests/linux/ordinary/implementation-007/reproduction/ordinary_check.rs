#![allow(dead_code)]
#[path = "src/deadline.rs"]
mod deadline;
#[path = "src/sys/linux/mod.rs"]
mod linux;

fn main() { std::process::exit(linux::ordinary_fixture::entry()); }
