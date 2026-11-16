//! Keys never reach stdout, stderr or artefacts (PLAN-R1 §2.3 Keys).
//! Every output line passes through `Redactor::apply`.

/// Env vars that hold built-in provider keys: one table (`keys::PROVIDERS`), so a new provider is redacted the day it
/// is added. `from_env` adds the keys.toml values and the variables of the user's `[[key]]` blocks too.
pub const KEY_VARS: [&str; crate::keys::PROVIDERS.len()] = {
    let mut v = [""; crate::keys::PROVIDERS.len()];
    let mut i = 0;
    while i < v.len() {
        v[i] = crate::keys::PROVIDERS[i].1;
        i += 1;
    }
    v
};

/// Values shorter than this are not redacted: replacing "a" everywhere helps no one.
const MIN_LEN: usize = 8;

pub struct Redactor {
    secrets: Vec<String>,
}

impl Redactor {
    pub fn new(values: impl IntoIterator<Item = String>) -> Self {
        let mut secrets: Vec<String> = values.into_iter().filter(|s| s.len() >= MIN_LEN).collect();
        // Longest first, so a key that contains another key is replaced whole.
        secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
        Self { secrets }
    }

    pub fn from_env() -> Self {
        Self::new(
            KEY_VARS
                .iter()
                .filter_map(|k| std::env::var(k).ok())
                .chain(crate::keys::all_values()),
        )
    }

    pub fn apply(&self, text: &str) -> String {
        self.secrets
            .iter()
            .fold(text.to_string(), |t, s| t.replace(s.as_str(), "***"))
    }
}

#[cfg(test)]
mod tests {
    use super::Redactor;

    #[test]
    fn replaces_keys_and_skips_short_values() {
        let r = Redactor::new([
            "sk-abcdef123456".into(),
            "sk-abcdef123456XYZ".into(),
            "ab".into(),
        ]);
        assert_eq!(
            r.apply("k=sk-abcdef123456XYZ;j=sk-abcdef123456;ab"),
            "k=***;j=***;ab"
        );
    }

    #[test]
    fn every_provider_key_variable_is_redacted() {
        for (_, var) in crate::keys::PROVIDERS {
            assert!(super::KEY_VARS.contains(&var), "{var} is not redacted");
        }
        assert!(
            super::KEY_VARS.contains(&"FAL_KEY")
                && super::KEY_VARS.contains(&"SCRIPTVOX_VISION_KEY")
        );
    }
}
