# Risk Register

Last updated: 2026-08-23

Priority: `P0` can cause unrecoverable loss or invalidate a release; `P1` is a significant pre-1.0 reliability/security risk; `P2` is bounded maturation work.

## Active risks

| ID | Pri | Residual risk | Current control | Next mitigation |
|---|---|---|---|---|
| R-01 | P0 | Losing an encryption key makes encrypted blobs unrecoverable. | Explicit key generation, restrictive key-file permissions, key-ID validation, and documented warnings. | Require a verified recovery copy before GUI enablement. |
| R-02 | P0 | No packaged, signed, notarized, or clean-machine-tested artifacts exist. | Source builds, tag/changelog validation, and Linux full-gate CI. | Add reproducible artifacts, provenance, platform signing, and install/restore/uninstall tests. |
| R-03 | P1 | Windows named-pipe ACL policy and runtime singleton behavior are not verified. | Pipe connection ordering is correct and Windows all-target/all-feature compilation runs in CI. | Define explicit per-user ACLs and test on clean Windows hosts. |
| R-04 | P1 | Atomic state replacement does not make cross-process read-modify-write updates transactional. | Same-directory temp, fsync, atomic replace, and daemon-local commit serialization/owned-field merges. | Add a state lease or transactional compare-and-swap protocol shared by every process. |
| R-05 | P1 | In-flight blocking filesystem work is not cooperatively cancellable. | Work leaves the async runtime and shutdown joins are bounded. | Add cancellation checkpoints to scan, restore, scrub, and replication primitives. |
| R-06 | P1 | Two backup engines remain public maintenance surfaces. | Production adapters use the versioned engine and the legacy exports are documented. | Migrate remaining consumers/tests and remove only under deprecation policy. |
| R-07 | P1 | Plaintext manifests expose source paths and metadata even with blob encryption. | Blob-only encryption scope is explicit in storage/security docs. | Design authenticated optional metadata encryption with migration/recovery semantics. |
| R-08 | P1 | Local malicious metadata replacement is detectable but not remotely attestable. | Strict manifest/index/path/hash validation, decoded-blob hashing, scrub, replication, and staged restore. | Add authenticated or signed manifest/index metadata. |
| R-09 | P1 | Tauri/Linux transitives emit 19 informational unmaintained/advisory warnings. | Current vulnerability scans report zero vulnerabilities and no Rust advisory ignore list exists. | Track GTK3/Wry/Tauri upgrades and reassess each warning on dependency changes. |
| R-10 | P2 | macOS/Windows compilation does not prove native service, IPC, dialog, or recovery behavior. | Hosted all-target/all-feature workspace compile jobs catch cfg/build drift. | Add platform runtime and clean-machine recovery suites. |
| R-11 | P2 | Coverage leaves destructive rollback, timeout, and platform branches lightly exercised. | Backend/frontend unit, integration, E2E, coverage, and performance gates. | Add interruption/property tests and raise thresholds based on stable coverage. |
| R-12 | P2 | Snapshot providers may require privileges and can fall back to live reads. | Snapshots are opt-in and timeout-bounded; fallback is reported and blob hashing rejects inconsistent reads. | Add platform capability checks and clearer preflight diagnostics. |
| R-13 | P2 | Fixed 30-minute GUI scheduling can overwrite an externally edited interval. | UI and configuration docs describe normalization. | Separate basic and advanced schedule ownership. |

## Established controls

- Live source mutation cannot publish bytes under a stale digest.
- Restore rejects hostile metadata, verifies decoded content, and stages before target replacement.
- Store-mutating operations use deterministic, bounded cross-process leases.
- Sampled scrub terminates and replication repairs incomplete indexed versions before index publication.
- Unix IPC uses private per-user endpoints and refuses unsafe live/stale endpoint replacement.
- State writes are atomic and GUI/daemon blocking work does not occupy async executor threads.
- The canonical quality gate includes tests, builds, coverage, performance, secret scans, and supply-chain audits.
- macOS and Windows compilation jobs cover conditional workspace code; CodeQL covers Rust and frontend code.
- npm/Rust vulnerability scans currently report zero vulnerabilities; stale Rust advisory suppressions were removed.

## Review triggers

Review this register when storage/encryption/restore semantics, platform support, public contracts, coverage thresholds, dependencies, or release packaging change. Roadmap work should reference the risks it reduces.
