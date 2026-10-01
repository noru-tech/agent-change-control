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

/// `acc` run from the repository root with no GitHub environment: no token, no
/// `GITHUB_REPOSITORY`, and no repository for `git` to find, so this checkout's own `origin`
/// remote is never inferred.
pub fn acc() -> Command {
    let mut cmd = Command::cargo_bin("acc").expect("acc binary");
    cmd.current_dir(root())
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_REPOSITORY")
        .env_remove("ACC_GITHUB_API_URL")
        .env_remove("ACC_NOW")
        .env("GIT_DIR", root().join("tests/no-such-git-dir"));
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

/// Lowercase hexadecimal SHA-256 of `bytes`, computed with `sha2` directly rather than through
/// `acc`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
