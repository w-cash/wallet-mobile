use std::{env, fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let lockfile = manifest_dir.join("../Cargo.lock");
    println!("cargo:rerun-if-changed={}", lockfile.display());

    let lock = fs::read_to_string(&lockfile).expect("read workspace Cargo.lock");
    let zingolib = lock
        .split("[[package]]")
        .find(|package| package.lines().any(|line| line == "name = \"zingolib\""))
        .expect("find zingolib package in workspace Cargo.lock");
    let source = zingolib
        .lines()
        .find_map(|line| line.strip_prefix("source = \""))
        .and_then(|line| line.strip_suffix('"'))
        .filter(|source| source.starts_with("git+https://github.com/w-cash/wallet-core.git?rev="))
        .expect("zingolib must use the reviewed Wcash wallet-core Git source");
    let revision = source
        .split("?rev=")
        .nth(1)
        .and_then(|value| value.split('#').next())
        .filter(|value| value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .expect("wallet-core revision must be a full 40-character Git hash");

    println!("cargo:rustc-env=WCASH_WALLET_CORE_REV={revision}");
}
