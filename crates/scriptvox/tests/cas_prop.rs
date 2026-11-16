use proptest::prelude::*;
use scriptvox::store::key;
use serde_json::{Map, Value};

proptest! {
    #[test]
    fn key_ignores_map_order(pairs in proptest::collection::vec(("[a-z]{1,6}", "[a-z0-9 ]{0,12}"), 1..8)) {
        let fwd: Map<String, Value> = pairs.iter().map(|(k, v)| (k.clone(), Value::from(v.clone()))).collect();
        let rev: Map<String, Value> = pairs.iter().rev().map(|(k, v)| (k.clone(), Value::from(v.clone()))).collect();
        // Same last-wins content either way only when keys are unique.
        let unique = pairs.iter().map(|(k, _)| k).collect::<std::collections::HashSet<_>>().len() == pairs.len();
        prop_assume!(unique);
        prop_assert_eq!(key("t", "1", &Value::Object(fwd)), key("t", "1", &Value::Object(rev)));
    }
}
