use std::process::{Child, Command};
use std::thread;
use std::time::Duration;
use reqwest::blocking::{Client, get};
use reqwest::{StatusCode};

struct SalsaTestServer {
    process: Child,
}

impl SalsaTestServer {
    fn spawn() -> Self {
        let backend_executable = env!("CARGO_BIN_EXE_backend");
        let server =    SalsaTestServer {
            process : Command::new(backend_executable)
                .spawn()
                .expect("Could not start backend")
        };
        while let Err(_) = get("http://127.0.0.1:3000/") {
            thread::sleep(Duration::from_millis(1));
            print!(".")
        }
        server
    }
}

impl Drop for SalsaTestServer {
    fn drop(&mut self) {
        self.process.kill().expect("Failed to send kill signal to backend");
        self.process.wait().expect("Backend failed to stop");
    }
}

#[test]
fn can_start_and_stop_backend() {
    SalsaTestServer::spawn();
}

#[test]
fn create_booking_not_logged_in() {
    let _server = SalsaTestServer::spawn();

    let client = Client::new();
    let res = client.post("http://127.0.0.1:3000/bookings")
        .form(
            &[
                ("start_date", "2025-07-01"),
                ("start_time", "02:00:00"),
                ("telescope", "fake1"),
                ("duration", "1"),
            ]).send().expect("Could not send request");

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
