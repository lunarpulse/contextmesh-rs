//! The contextmesh automation CLI and sync daemon.

fn main() -> std::process::ExitCode {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let args: Vec<String> = std::env::args().skip(1).collect();
    runtime.block_on(contextmesh::cli::run(&args))
}
