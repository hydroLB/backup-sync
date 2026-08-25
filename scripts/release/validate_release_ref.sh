#!/usr/bin/env bash
set -euo pipefail

REF_NAME="${1:-}"
MAIN_REF="${2:-origin/main}"

if [[ -z "${REF_NAME}" ]]; then
  echo "validate_release_ref.sh: missing ref argument (example: v1.2.3 or HEAD)." >&2
  exit 1
fi

if ! git rev-parse --verify "${REF_NAME}^{commit}" >/dev/null 2>&1; then
  echo "validate_release_ref.sh: ref does not resolve to a commit: ${REF_NAME}" >&2
  exit 1
fi

commit_ref="${REF_NAME}^{commit}"
if [[ "${REF_NAME}" != "HEAD" ]]; then
  if [[ "$(git cat-file -t "${REF_NAME}")" != "tag" ]]; then
    echo "validate_release_ref.sh: release ref must be an annotated tag: ${REF_NAME}" >&2
    exit 1
  fi
fi

if ! git rev-parse --verify "${MAIN_REF}^{commit}" >/dev/null 2>&1; then
  echo "validate_release_ref.sh: main ref not found locally: ${MAIN_REF}" >&2
  echo "Fetch it first, for example: git fetch origin main --force" >&2
  exit 1
fi

if ! git merge-base --is-ancestor "${commit_ref}" "${MAIN_REF}"; then
  echo "validate_release_ref.sh: ${REF_NAME} does not point to a commit reachable from ${MAIN_REF}" >&2
  exit 1
fi

echo "Release ref validated: ${REF_NAME} is annotated when required and reachable from ${MAIN_REF}"
