use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn test_script() -> &'static str {
    if cfg!(feature = "ci") {
        "scripts/ci/ios_integration_tests_ci.sh"
    } else {
        "scripts/ios_integration_tests.sh"
    }
}

fn run_xctest(arguments: &[&str]) -> Output {
    Command::new(repository_root().join(test_script()))
        .args(arguments)
        .current_dir(repository_root())
        .output()
        .unwrap()
}

fn require_success(run: Output) {
    assert!(
        run.status.success(),
        "iOS XCTest failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn ios_sources_select_the_wcash_regtest_boundary() {
    let root = repository_root();
    let swift = fs::read_to_string(root.join("ios/ZingoTests/ZingoTest.swift")).unwrap();
    let required = [
        "http://127.0.0.1:48234",
        concat!("w", "u", "regtest1"),
        "c3a6678a",
        "TWC",
        "throw XCTSkip",
    ];
    for token in required {
        assert!(swift.contains(token), "the iOS suite is missing {token}");
    }
    for token in [
        "http://10.0.2.2:20000",
        "http://127.0.0.1:20000",
        "uviewregtest1",
        "texregtest1",
        "t1dUDJ62",
        "u1gsqvq",
    ] {
        assert!(!swift.contains(token), "the iOS suite contains {token}");
    }

    let plist = fs::read_to_string(root.join("ios/Zingo/Info.plist")).unwrap();
    assert!(plist.contains("<string>LaunchScreen</string>"));
    assert!(root.join("ios/Zingo/LaunchScreen.storyboard").is_file());
}

#[test]
#[ignore = "requires full Xcode and an installed iOS simulator"]
fn ios_xctest_suite() {
    require_success(run_xctest(&["-e", "ZingoTests"]));
}

#[test]
#[ignore = "requires full Xcode plus Wcash CompactTxStreamer on 127.0.0.1:48234"]
fn ios_local_regtest_suite() {
    require_success(run_xctest(&["-l"]));
}
