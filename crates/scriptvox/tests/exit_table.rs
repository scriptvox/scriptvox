use scriptvox::exit::Exit;

#[test]
fn exit_table_is_locked() {
    let table: Vec<String> = Exit::ALL
        .iter()
        .map(|e| format!("{:>2} {:<18} {}", e.code(), e.name(), e.meaning()))
        .collect();
    assert_eq!(table.len(), 12);
    insta::assert_snapshot!(table.join("\n"));
}

#[test]
fn codes_are_unique() {
    let mut codes: Vec<u8> = Exit::ALL.iter().map(|e| e.code()).collect();
    codes.sort();
    codes.dedup();
    assert_eq!(codes.len(), Exit::ALL.len());
}
