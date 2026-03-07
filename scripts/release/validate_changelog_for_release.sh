#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-}"
CHANGELOG_FILE="${2:-CHANGELOG.md}"

if [[ -z "${VERSION}" ]]; then
  echo "validate_changelog_for_release.sh: missing version argument (example: 1.2.3)." >&2
  exit 1
fi

if [[ ! -f "${CHANGELOG_FILE}" ]]; then
  echo "validate_changelog_for_release.sh: changelog file not found: ${CHANGELOG_FILE}" >&2
  exit 1
fi

if ! grep -Eq "^## \[Unreleased\]$" "${CHANGELOG_FILE}"; then
  echo "validate_changelog_for_release.sh: missing required '## [Unreleased]' section." >&2
  exit 1
fi

RELEASE_HEADING_REGEX="^## \[${VERSION//./\\.}\] - [0-9]{4}-[0-9]{2}-[0-9]{2}$"
if ! grep -Eq "${RELEASE_HEADING_REGEX}" "${CHANGELOG_FILE}"; then
  echo "validate_changelog_for_release.sh: missing release section for ${VERSION} with date." >&2
  echo "Expected heading format: ## [${VERSION}] - YYYY-MM-DD" >&2
  exit 1
fi

if ! awk -v version="${VERSION}" '
  BEGIN { in_section=0; has_bullet=0; }
  $0 ~ "^## \\[" version "\\] - " { in_section=1; next }
  /^## \[/ && in_section { exit }
  in_section && /^- / { has_bullet=1 }
  END { exit(has_bullet ? 0 : 1) }
' "${CHANGELOG_FILE}"; then
  echo "validate_changelog_for_release.sh: release section ${VERSION} has no bullet entries." >&2
  exit 1
fi

echo "Changelog section validated for release ${VERSION}: ${CHANGELOG_FILE}"
