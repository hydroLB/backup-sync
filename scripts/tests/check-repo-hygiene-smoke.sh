#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SOURCE_SCRIPT="${ROOT_DIR}/scripts/check-repo-hygiene.sh"

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "[hygiene-smoke] required command not found: $cmd" >&2
    exit 1
  fi
}

require_cmd git
require_cmd rg
require_cmd mktemp

make_fixture_repo() {
  local dir="$1"
  mkdir -p "${dir}/scripts" "${dir}/crates/gui/frontend"
  cp "${SOURCE_SCRIPT}" "${dir}/scripts/check-repo-hygiene.sh"
  chmod +x "${dir}/scripts/check-repo-hygiene.sh"
  cat <<'EOF' > "${dir}/README.md"
# Fixture
EOF
  cat <<'EOF' > "${dir}/SECURITY.md"
# Fixture
EOF
  cat <<'EOF' > "${dir}/.gitignore"
target/
EOF
  cat <<'EOF' > "${dir}/.env.example"
EXAMPLE=1
EOF
  : > "${dir}/Cargo.lock"
  : > "${dir}/crates/gui/frontend/package-lock.json"
  (
    cd "${dir}"
    git init -q
    git config user.email fixture@example.com
    git config user.name fixture
    git add .
  )
}

assert_passes() {
  local dir="$1"
  (
    cd "${dir}"
    ./scripts/check-repo-hygiene.sh >/dev/null
  )
}

assert_fails_with() {
  local dir="$1"
  local pattern="$2"
  local output
  set +e
  output="$(
    cd "${dir}" &&
      ./scripts/check-repo-hygiene.sh 2>&1
  )"
  local status=$?
  set -e
  if [[ ${status} -eq 0 ]]; then
    echo "[hygiene-smoke] expected failure but script passed" >&2
    exit 1
  fi
  if ! printf '%s\n' "${output}" | rg -q "${pattern}"; then
    echo "[hygiene-smoke] failure output did not match pattern: ${pattern}" >&2
    printf '%s\n' "${output}" >&2
    exit 1
  fi
}

tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

case_deleted_tracked="${tmpdir}/deleted-tracked"
make_fixture_repo "${case_deleted_tracked}"
cat <<'EOF' > "${case_deleted_tracked}/notes.txt"
/Users/me/example
EOF
(
  cd "${case_deleted_tracked}"
  git add notes.txt
  rm notes.txt
)
assert_passes "${case_deleted_tracked}"

case_self_and_placeholder="${tmpdir}/self-and-placeholder"
make_fixture_repo "${case_self_and_placeholder}"
cat <<'EOF' > "${case_self_and_placeholder}/docs.txt"
/Users/me/Projects
/home/user/project
C:\Users\user\project
EOF
(
  cd "${case_self_and_placeholder}"
  git add docs.txt
)
assert_passes "${case_self_and_placeholder}"

case_real_machine_path="${tmpdir}/real-machine-path"
make_fixture_repo "${case_real_machine_path}"
printf '/%s/%s/Secrets\n' 'Users' 'alice' > "${case_real_machine_path}/docs.txt"
(
  cd "${case_real_machine_path}"
  git add docs.txt
)
assert_fails_with "${case_real_machine_path}" "machine-specific absolute paths detected"

case_generated_comment="${tmpdir}/generated-comment"
make_fixture_repo "${case_generated_comment}"
cat <<'EOF' > "${case_generated_comment}/crates/generated.rs"
/// Summary: Generated narration.
fn generated() {}
EOF
assert_fails_with "${case_generated_comment}" "generated comment templates detected"

echo "[hygiene-smoke] OK"
