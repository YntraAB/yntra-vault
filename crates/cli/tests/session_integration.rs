use std::{io::Write, process::{Command, Stdio}};

fn cli() -> Command { Command::new(env!("CARGO_BIN_EXE_yntra")) }

#[test]
fn native_host_returns_framed_errors_and_current_version() {
    let mut child = cli().env("YNTRA_SESSION", "synthetic-framing-test-session").args(["native-host", "run"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut input = child.stdin.take().unwrap();
    for value in ["{", "{}", r#"{"command":"ping"}"#] {
        input.write_all(&(value.len() as u32).to_ne_bytes()).unwrap();
        input.write_all(value.as_bytes()).unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let mut cursor = 0;
    for expected in ["error", "error", "ok"] {
        let length = u32::from_ne_bytes(output.stdout[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;
        let value: serde_json::Value = serde_json::from_slice(&output.stdout[cursor..cursor + length]).unwrap();
        cursor += length;
        assert_eq!(value["status"], expected);
        if expected == "ok" { assert_eq!(value["version"], env!("CARGO_PKG_VERSION")); }
    }
    assert_eq!(cursor, output.stdout.len());
    let invalid = cli().args(["native-host", "install", "chrome"]).output().unwrap();
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("--extension-id"));
}

#[cfg(windows)]
#[test]
fn lock_authenticates_before_removing_the_isolated_handoff_token() {
    use std::{thread, time::{Duration, Instant}};
    let directory = tempfile::tempdir().unwrap();
    let vault = directory.path().join("fixture.vdb");
    let password = "Synthetic isolated session test";
    yntra_vault_core::vault::VaultManager::create("Session fixture", password, &vault).unwrap();
    let username = format!("YntraTest-{}", uuid::Uuid::new_v4());
    let command = || {
        let mut command = cli();
        command.env("LOCALAPPDATA", directory.path()).env("USERNAME", &username)
            .env("YNTRA_PASSWORD", password).env_remove("YNTRA_SESSION").env_remove("YNTRA_VAULT_PATH");
        command
    };
    struct Guard(std::process::Child);
    impl Drop for Guard { fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); } }
    let mut daemon = Guard(command().arg("--path").arg(&vault).arg("unlock").stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap());
    let token = directory.path().join("Yntra Vault/session.token");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !token.exists() && Instant::now() < deadline { thread::sleep(Duration::from_millis(20)); }
    assert!(token.exists());
    thread::sleep(Duration::from_millis(100));
    let wrong = command().env("YNTRA_SESSION", "wrong-test-token").arg("lock").output().unwrap();
    assert!(!wrong.status.success());
    assert!(token.exists());
    assert!(daemon.0.try_wait().unwrap().is_none());
    let locked = command().arg("lock").output().unwrap();
    assert!(locked.status.success(), "{}", String::from_utf8_lossy(&locked.stderr));
    assert!(!token.exists());
    let deadline = Instant::now() + Duration::from_secs(5);
    while daemon.0.try_wait().unwrap().is_none() && Instant::now() < deadline { thread::sleep(Duration::from_millis(20)); }
    assert!(daemon.0.try_wait().unwrap().is_some(), "Authenticated lock must terminate the daemon");
}
