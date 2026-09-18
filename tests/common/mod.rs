#![allow(dead_code)]

use std::path::PathBuf;

use assert_cmd::Command;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn fixtures() -> PathBuf {
    root().join("tests/fixtures")
}

pub fn fixture(dir: &str, file: &str) -> PathBuf {
    fixtures().join(dir).join(file)
}

/// `acc` run from the repository root with no GitHub environment.
pub fn acc() -> Command {
    let mut cmd = Command::cargo_bin("acc").expect("acc binary");
    cmd.current_dir(root())
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_REPOSITORY");
    cmd
}

pub fn stdout(cmd: &mut Command) -> String {
    let out = cmd.output().expect("run acc");
    assert!(
        out.status.success(),
        "acc failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf-8 stdout")
}
