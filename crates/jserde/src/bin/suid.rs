//! Compute the JDK default `serialVersionUID` from a `.class` file.
//!
//! Usage: `cargo run -p jserde --bin suid -- path/to/Foo.class`

use std::process::ExitCode;

use jserde::suid::{compute_default_suid, parse_class_file};

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: suid <path/to/Class.class>");
        return ExitCode::FAILURE;
    };

    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("suid: cannot read {path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let signature = match parse_class_file(&bytes) {
        Ok(signature) => signature,
        Err(error) => {
            eprintln!("suid: {error}");
            return ExitCode::FAILURE;
        }
    };
    match compute_default_suid(&signature) {
        Ok(suid) => {
            println!("{suid}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("suid: {error}");
            ExitCode::FAILURE
        }
    }
}
