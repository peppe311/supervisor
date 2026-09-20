# Open-source publication record

Date: 2026-09-21. Repository: https://github.com/peppe311/supervisor.
The replacement publication starts with a fresh source history. Earlier
publication commits and pull-request references are not imported.

## Source and licensing

- A new Git history is prepared from 4,661 selected source/resource files
  (28.4 MiB). The development repository and its history were preserved.
- Build outputs, logs, account data, chats, private profiles, downloaded official
  web-document snapshots and the old design-delivery ZIP were excluded.
- Original code retains MPL-2.0. Upstream contracts, vendored crates, fonts and
  icons retain their own licenses and attributions.
- The inventory contains 775 components and 413 distinct notice texts. All 498
  required Windows/frontend/asset entries have collected notices. Optional
  dependencies for other operating systems are identified separately; the
  non-Windows `dispatch` crate has metadata-only coverage and is not shipped.
- Collected notices and a passing policy check do not constitute a legal
  certification or completion of the production binary release gate.

## Local validation

- Baseline workspace verification passed: 828 Rust tests and 216 Node tests.
  The privacy correction also passed seven publication checks, ten history
  presentation tests, seven profile-isolation tests, ten SSH fixture tests and
  all four supported protocol contracts. One live SSH test remained ignored.
- 25 Rust tests remained ignored; live accounts, desktop interaction and remote
  systems were not exercised as part of this publication.
- Frontend type/design checks, four supported Codex schema contracts, Rust
  formatting/check/Clippy, packaging/storage checks and notice drift checks passed.
- Gitleaks 8.30.1 scanned all 11 development commits and the selected current
  source files. Two historical, synthetic examples have exact fingerprint
  exceptions. The fresh public-source history also passed without findings.
- Actionlint 1.7.12 and PowerShell parse checks passed.
- The npm runtime dependency audit reported zero vulnerabilities. Cargo audit
  passed with the existing, documented RSA advisory exception and four visible
  warnings for dependencies outside the Windows target. See DEPENDENCY_SECURITY.md.
- The package inventory generator was exercised and retained the complete
  Windows notice coverage and package hashes.

## Hosted validation and release

The initial hosted run exposed a Windows checkout issue: `.svelte` and `.ts`
files had no explicit end-of-line attributes. Explicit LF rules preserve the
same source layout on a clean Windows checkout and in local builds. Publication
proceeds only after the corrected source passes hosted verification.

Hosted Windows runs also exposed an overly short ten-second test-harness
deadline for the 900 KB output-integrity fixture. That fixture now has a
bounded sixty-second drain deadline and reports byte/event counts on failure.
Its complete-output, pagination and truncation assertions remain unchanged.
Production process limits and the separate timeout/cancellation tests retain
their original deadlines. No production-signed executable is part of the
source release.

The checked-in workflows and branch policy, contribution/security documents,
Dependabot configuration and release notes are in the source commit. Raw local
scan reports and temporary tooling are excluded from publication. Temporary
tool cleanup was blocked by the local automatic approval policy; these files
remain only under the ignored development `target/publication/` directory.

## Follow-up privacy review

A review after the first public release found conversation titles, native
identifiers and account-specific import details in historical engineering
documentation. The secret scan did not identify these non-secret personal
metadata. The repository was immediately made private again.

The expanded review covered all 4,660 historical paths across the three source
commits and ten Dependabot commits. No transcript exports, personal databases,
account profiles, photographs or credential files were found. Gitleaks scanned
all thirteen commits without a credential finding. The three published PNGs were
application artwork without EXIF/text metadata. Fixture messages were synthetic.

The local candidate replaces the account-specific profile audit with generic
technical documentation, removes disposable native identifiers from the plan,
uses synthetic fixture identifiers/profile paths, and adds privacy checks to
export preparation and publication validation. Transcript/database export paths
are also explicitly rejected. Secret scanning and manual content review remain
separate requirements.

The user approved preserving the first repository as a private archive and
creating a separate repository from this corrected snapshot. Its old commits,
tags and pull-request references are not copied into the replacement. Public
visibility is gated on passing hosted tests for the new source commit. Making
the first repository private does not revoke copies obtained while it was
publicly available.
