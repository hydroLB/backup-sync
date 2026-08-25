use super::model::VersionIndex;
use anyhow::{Context, Result};

/// Accept only identifiers that can be embedded safely in store artifact names.
pub(crate) fn validate_version_id(version_id: &str) -> Result<()> {
    if version_id.is_empty()
        || version_id.len() > 255
        || !version_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        anyhow::bail!(
            "versioned::validate_version_id invalid version identifier {:?}",
            version_id
        );
    }
    Ok(())
}

/// Validate every identifier loaded from an untrusted on-disk version index.
pub(crate) fn validate_version_index(index: &VersionIndex) -> Result<()> {
    for (position, version) in index.versions.iter().enumerate() {
        validate_version_id(&version.id).with_context(|| {
            format!("versioned::validate_version_index invalid versions[{position}].id")
        })?;
    }
    if let Some(version_id) = index.safety_pinned_version_id.as_deref() {
        validate_version_id(version_id)
            .context("versioned::validate_version_index invalid safety_pinned_version_id")?;
    }
    if let Some(version_id) = index.safety_pending_version_id.as_deref() {
        validate_version_id(version_id)
            .context("versioned::validate_version_index invalid safety_pending_version_id")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::versioned::model::VersionInfo;

    fn index_with(version_id: &str) -> VersionIndex {
        VersionIndex {
            versions: vec![VersionInfo {
                id: version_id.to_string(),
                created_at_unix: 1,
            }],
            ..VersionIndex::default()
        }
    }

    #[test]
    fn accepts_generated_and_legacy_safe_identifiers() {
        validate_version_id("20260823-123456-123456789-1").expect("generated id");
        validate_version_id("legacy_V1").expect("legacy safe id");
    }

    #[test]
    fn rejects_unsafe_ids_in_every_index_field() {
        let mut index = index_with("../escape");
        assert!(validate_version_index(&index).is_err());

        index.versions[0].id = "safe".to_string();
        index.safety_pinned_version_id = Some("../escape".to_string());
        assert!(validate_version_index(&index).is_err());

        index.safety_pinned_version_id = None;
        index.safety_pending_version_id = Some("../escape".to_string());
        assert!(validate_version_index(&index).is_err());
    }
}
