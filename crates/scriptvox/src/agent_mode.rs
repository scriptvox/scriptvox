//! Agent mode (PLAN-R1 §2.3). Only env vars switch it on: a missing TTY alone
//! does not, because Git Bash and mintty give native programs a pipe.

/// Env vars that mean "an agent or CI runs this command". One row for each variable, with the tool that sets it
/// for the commands of its shell tool. The last three were read in the code of each tool on 13 Nov 2026
/// (opencode 1.18.35, Hermes Agent 0.21.1, Pi 0.78.0). opencode also sets `AGENT=1`: that name is too general
/// to be a row here.
pub const VARS: [&str; 6] = [
    "CLAUDECODE",             // Claude Code
    "CLAUDE_CODE_ENTRYPOINT", // Claude Code
    "CI",                     // CI systems
    "OPENCODE",               // opencode: `OPENCODE=1`
    "HERMES_AGENT",           // Hermes Agent: `HERMES_AGENT=true`
    "PI_CODING_AGENT",        // Pi: `PI_CODING_AGENT=true`
];

/// True when any agent var is set to a value other than empty, `0` or `false`.
pub fn detect(get: impl Fn(&str) -> Option<String>) -> bool {
    VARS.iter().any(|v| {
        get(v).is_some_and(|s| {
            let s = s.trim();
            !(s.is_empty() || s == "0" || s.eq_ignore_ascii_case("false"))
        })
    })
}

pub fn from_env() -> bool {
    detect(|k| std::env::var(k).ok())
}

#[cfg(test)]
mod tests {
    use super::detect;

    #[test]
    fn values() {
        let on = |k: &'static str, v: &'static str| move |q: &str| (q == k).then(|| v.to_string());
        assert!(detect(on("CLAUDECODE", "1")));
        assert!(detect(on("CI", "true")));
        assert!(detect(on("CLAUDE_CODE_ENTRYPOINT", "cli")));
        assert!(detect(on("OPENCODE", "1")));
        assert!(detect(on("HERMES_AGENT", "true")));
        assert!(detect(on("PI_CODING_AGENT", "true")));
        assert!(!detect(on("AGENT", "1")));
        assert!(!detect(on("CI", "false")));
        assert!(!detect(on("CI", "0")));
        assert!(!detect(on("CI", "")));
        assert!(!detect(|_| None));
    }
}
