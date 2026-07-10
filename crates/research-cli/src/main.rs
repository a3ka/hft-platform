//! CLI: research-cli grid|validate|report (FA §3 outer). std::env::args, без clap.
//! Реализация — research-dev (M-04 task 4).

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("grid") | Some("validate") | Some("report") => {
            todo!("research-dev: M-04 task 4")
        }
        _ => {
            eprintln!("usage: research-cli <grid|validate|report> ...");
            std::process::exit(2);
        }
    }
}
