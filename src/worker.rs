#![forbid(unsafe_code)]
use photocraft_background_removal_plugin::{Result, engine, error, protocol};
use std::{
    io::Read,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--job")) {
        return Err(error("Expected --job <request.json>"));
    }
    let path = args.next().ok_or_else(|| error("Missing job request."))?;
    if args.next().is_some() {
        return Err(error("Unexpected worker argument."));
    }
    let job = protocol::parse_job(std::fs::File::open(path)?)?;
    protocol::validate_input(&job)?;
    let root = std::env::current_exe()?;
    let runtime = root
        .parent()
        .ok_or_else(|| error("Cannot locate plugin installation."))?
        .join("runtime");
    let cancel = Arc::new(AtomicBool::new(false));
    let lifetime = Arc::clone(&cancel);
    // EOF or any byte cancels; the editor also kills and reaps this process if needed.
    // Do not join this reader: successful completion can occur while host stdin is open.
    std::thread::Builder::new()
        .name("background-host-lifetime".into())
        .spawn(move || {
            let mut byte = [0u8; 1];
            let _ = std::io::stdin().read(&mut byte);
            lifetime.store(true, Ordering::Relaxed);
        })?;
    engine::remove_background(
        &job.input,
        &job.output,
        &job.options,
        &runtime,
        &cancel,
        &|progress, message| {
            eprintln!(
                "{}",
                serde_json::json!({"progress":progress,"message":message})
            );
        },
    )
}

fn main() {
    std::panic::set_hook(Box::new(|_| {
        eprintln!("{{\"error\":\"Native engine initialization failed.\"}}")
    }));
    if let Err(e) = run() {
        // Avoid unbounded native error diagnostics on the host's progress pipe.
        let message: String = e.to_string().chars().take(2048).collect();
        eprintln!("{}", serde_json::json!({"error":message}));
        std::process::exit(1);
    }
}
