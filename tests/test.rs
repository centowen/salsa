use std::process::Command;

#[test]
fn can_start_and_stop_backend() {
    let backend_executable = env!("CARGO_BIN_EXE_backend");
    println!("Running `backend` {backend_executable}");
    let mut process = Command::new(backend_executable)
        .spawn()
        .expect("Could not start backend");

    Command::new("kill")
        .args(["-s", "TERM", &process.id().to_string()])
        .status()
        .expect("Failed to send signal");

    process.wait().expect("backend failed to stop");
}
