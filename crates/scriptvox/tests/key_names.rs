//! A key declared in the overlay works like a built-in one, and its value stays out of every output.
mod common;
use std::io::Write;

const MADE_UP: &str = "made-up-value-not-a-real-key-1234";
const MADE_UP_ENV: &str = "made-up-env-value-not-a-real-key-5678";

#[test]
fn a_declared_key_is_listed_stored_and_redacted() {
    let dir = tempfile::tempdir().unwrap();
    let mut f = std::fs::File::create(dir.path().join("generators.toml")).unwrap();
    writeln!(
        f,
        "[[key]]\nname = \"acme\"\nenv = \"ACME_TEST_KEY\"\n[[key]]\nname = \"bad\"\nenv = \"PATH\""
    )
    .unwrap();
    let rvx = |args: &[&str], stdin: &str| {
        let out = common::bin()
            .env("SCRIPTVOX_CONFIG", dir.path())
            .env_remove("SCRIPTVOX_GENERATORS")
            .env_remove("ACME_TEST_KEY")
            .args(args)
            .write_stdin(stdin)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    let set = rvx(&["keys", "set", "acme", "--stdin", "--json"], MADE_UP);
    assert!(set.contains("\"stored\""), "{set}");
    assert!(rvx(&["keys", "set", "bad", "--stdin", "--json"], MADE_UP).contains("unknown provider"));
    let list = rvx(&["keys", "list", "--json"], "");
    assert!(
        list.contains("ACME_TEST_KEY") && list.contains("keys.toml"),
        "{list}"
    );
    let providers = rvx(&["providers", "--json"], "");
    assert!(
        providers.contains("env \\\"PATH\\\" is not a key variable"),
        "{providers}"
    );
    for out in [&set, &list, &providers] {
        assert!(!out.contains(MADE_UP));
    }

    // This file is its own test process with this one test, so changing the environment races with nothing.
    unsafe {
        std::env::set_var("SCRIPTVOX_CONFIG", dir.path());
        std::env::remove_var("SCRIPTVOX_GENERATORS");
        std::env::set_var("ACME_TEST_KEY", MADE_UP_ENV);
    }
    let r = scriptvox::redact::Redactor::from_env();
    assert_eq!(r.apply(MADE_UP), "***");
    assert_eq!(r.apply(MADE_UP_ENV), "***");
}

/// Every test starts `scriptvox` through `common::bin`, `common::std_bin` or `common::clean`: no key of the shell and
/// no folder of the user reaches a child (`docs/specs/contributor-ci.md`).
#[test]
fn no_test_starts_the_binary_with_the_environment_of_the_shell() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut bad = vec![];
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        let Ok(text) = std::fs::read_to_string(&p) else {
            continue;
        };
        // Written in two parts, so this file does not find itself.
        if [concat!("cargo_", "bin("), concat!("CARGO_BIN_", "EXE")]
            .iter()
            .any(|n| text.contains(n))
        {
            bad.push(p.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert!(bad.is_empty(), "use common::bin() in {bad:?}");
}
