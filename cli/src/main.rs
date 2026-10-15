#![forbid(unsafe_code)]

mod args;
mod contracts;
mod core;
mod flags;
mod furniture;
mod host;

use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(core::run(std::env::args_os().skip(1)))
}
