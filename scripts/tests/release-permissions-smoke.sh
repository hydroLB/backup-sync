#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
workflow="${repo_root}/.github/workflows/release.yml"

top_level="$(awk '/^jobs:/{exit} {print}' "${workflow}")"
validate_job="$(awk '/^  validate:/{capture=1} /^  publish:/{capture=0} capture {print}' "${workflow}")"
publish_job="$(awk '/^  publish:/{capture=1} capture {print}' "${workflow}")"

if ! grep -Fq 'contents: read' <<<"${top_level}"; then
  echo '[release-permissions] workflow default must be contents: read' >&2
  exit 1
fi
if ! grep -Fq 'contents: read' <<<"${validate_job}"; then
  echo '[release-permissions] validate job must explicitly use contents: read' >&2
  exit 1
fi
if ! grep -Fq 'contents: write' <<<"${publish_job}"; then
  echo '[release-permissions] publish job must explicitly use contents: write' >&2
  exit 1
fi
if [[ "$(grep -cF 'contents: write' "${workflow}")" -ne 1 ]]; then
  echo '[release-permissions] only the publish job may use contents: write' >&2
  exit 1
fi

echo '[release-permissions] release workflow permissions are least-privilege'
