#!/usr/bin/env bash
set -euo pipefail

TAG="${1:-}"

if [[ -z "${TAG}" ]]; then
  echo "validate_semver_tag.sh: missing tag argument (example: v1.2.3)" >&2
  exit 1
fi

if [[ ! "${TAG}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-rc\.[0-9]+)?$ ]]; then
  echo "validate_semver_tag.sh: invalid tag '${TAG}'." >&2
  echo "Expected: vMAJOR.MINOR.PATCH or vMAJOR.MINOR.PATCH-rc.N" >&2
  exit 1
fi

echo "SemVer tag validated: ${TAG}"
