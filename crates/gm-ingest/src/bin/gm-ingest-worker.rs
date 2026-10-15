//! The ingestion worker (MODELS.md 6.2): one upload, parsed in a process of its own under
//! resource limits, so that a hostile file costs one upload and never the hub. The hub starts
//! `gm-ingest-worker --frame NAME --in FILE --out DIR` beside itself and reads what it writes.
//! A plain synchronous process: no runtime, no logging, no database.

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut frame = None;
    let mut input = None;
    let mut out = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--frame" => frame = it.next().and_then(|f| gm_model::rig::frame_by_name(f)),
            "--in" => input = it.next().map(PathBuf::from),
            "--out" => out = it.next().map(PathBuf::from),
            _ => {}
        }
    }
    let (Some(frame), Some(input), Some(out)) = (frame, input, out) else {
        eprintln!("gm-ingest-worker --frame NAME --in FILE --out DIR");
        std::process::exit(2);
    };
    #[cfg(unix)]
    {
        // 2 GiB of address space and 20 s of CPU: a bomb dies here, not in the hub.
        let _ = rlimit::setrlimit(rlimit::Resource::AS, 2 << 30, 2 << 30);
        let _ = rlimit::setrlimit(rlimit::Resource::CPU, 20, 20);
        let _ = rlimit::setrlimit(rlimit::Resource::CORE, 0, 0);
    }
    match gm_ingest::worker(&input, frame, &out) {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            eprintln!("gm-ingest-worker: {e}");
            std::process::exit(1);
        }
    }
}
