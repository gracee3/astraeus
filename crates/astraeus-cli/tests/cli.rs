use std::process::Command;

#[test]
fn missing_command_reports_usage() {
    let output = Command::new(env!("CARGO_BIN_EXE_astraeus"))
        .output()
        .expect("run Astraeus CLI");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage:"));
}
