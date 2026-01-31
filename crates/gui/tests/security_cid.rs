use gui::commands::security::cid;

#[test]
/// Purpose: Ensure correlation ids include prefixes and remain unique.
///
/// Inputs: None.
/// Outputs: Asserts prefix and uniqueness of correlation ids.
/// Ties to: `gui::commands::security::cid`.
/// Side effects: None.
/// Why: Keep traceability consistent across GUI command calls.
fn cid_prefix_and_unique() {
    let a = cid("test", None);
    let b = cid("test", None);
    assert!(
        a.starts_with("test-"),
        "security_cid::cid_prefix_and_unique missing prefix"
    );
    assert_ne!(
        a, b,
        "security_cid::cid_prefix_and_unique expected unique cids"
    );
}
