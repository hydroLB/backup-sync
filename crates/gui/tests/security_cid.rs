use gui::observability::cid;

#[test]
/// Summary: Ensure correlation ids include prefixes and remain unique.
///
/// Inputs: None.
///
/// Outputs: Asserts prefix and uniqueness of correlation ids.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `gui::observability::cid`.
///
/// Why this exists: Keep traceability consistent across GUI command calls.
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
