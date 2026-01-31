use gui::commands::security;

#[test]
/// Purpose: Ensure auth correlation ids include the auth prefix and are unique.
///
/// Inputs: None.
/// Outputs: Asserts prefix and uniqueness for auth correlation ids.
/// Ties to: `gui::commands::security::cid`.
/// Side effects: None.
/// Why: Prevent regressions in auth correlation id generation.
fn cid_prefix_changes() {
    let a = security::cid("auth", None);
    let b = security::cid("auth", None);
    assert!(
        a.starts_with("auth-"),
        "auth_unlock::cid_prefix_changes missing prefix"
    );
    assert_ne!(a, b, "auth_unlock::cid_prefix_changes expected unique cids");
}
