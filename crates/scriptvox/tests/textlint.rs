use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn random_arabic_words_give_no_findings(words in proptest::collection::vec("[ء-ي]{2,9}", 1..30)) {
        let text = words.join(" ");
        prop_assert!(scriptvox::textlint::check(&text).is_empty(), "{text}");
    }
}
