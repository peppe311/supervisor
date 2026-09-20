# Publishing Supervisor

## First source publication

1. Review `LICENSES.md`, update notices with `scripts/update-licenses.ps1` and run
   `scripts/verify-workspace.ps1`. Record what was actually tested. Review prose
   and fixtures separately for personal data: secret scanners do not identify
   conversation titles, native IDs or account-specific audit narratives.
2. Run `scripts/scan-secrets.ps1`. It checks all Git history plus a staged copy of
   eligible current sources, including untracked source files. Reports redact
   matched values and are deleted with the temporary build session.
3. Run `scripts/prepare-publication.ps1`. The fixed `target/publication/source/`
   snapshot excludes the development `.git`, executables, caches, logs, local
   credentials/data, old brand archives and full copied OpenAI documentation.
   Canonical artwork and original engineering studies are retained. The adjacent
   manifest records transferred file hashes. The existing development repository
   and production app data are untouched.
4. Initialize a new Git repository in that snapshot with a `main` branch and a
   fresh initial commit. Run `node scripts/verify-publication.mjs` there and scan
   the snapshot again. Never reuse the development history as the initial push.
5. Create the GitHub repository privately, push `main`, and inspect CI before
   changing visibility. Recheck publication contents before making it public.
6. Enable branch protection, dependency vulnerability alerts, security fixes and
   private vulnerability reporting. The checked-in branch-protection policy
   requires PRs, passing Windows/secret checks, resolved conversations and no
   force pushes. It allows a solo maintainer to merge without another person's
   approval. Some private-repository controls depend on the owner's GitHub plan;
   verify API results rather than claiming they are enabled.
7. Tag the reviewed commit `v0.1.0-alpha.2`. The tag workflow runs CI and creates
   an **unpublished draft prerelease of source only**. Review and publish that
   draft deliberately; it never uploads the local unsigned executable.

To update an existing owned snapshot, rerun the preparation script. It refuses
unknown files and independent source edits before replacing previously copied
files, and preserves the snapshot's independent `.git`. Do not develop in both
trees simultaneously. After the initial publication, prefer a normal clone/PR
workflow for changes to the public repository.

## Automation and review

CI uses immutable action commit hashes, read-only default permissions, a full
history secret scan and no stored provider credentials. Pull requests from
forks run with restricted permissions. Dependabot opens weekly Cargo, npm and
Actions updates; dependency updates still need refreshed notices and review.
Secrets scanning is not a guarantee that no private information exists: inspect
documentation, binary assets and any new sample data before publication.
The export and layout checks also reject non-synthetic native identifiers and
personal Windows profile paths. Use explicit fixture identifiers and paths.
These checks do not replace a review of human-readable content or Git history.

If personal content has already been published, restrict repository visibility
first. Removing it in a later commit does not remove earlier commits, tags,
pull-request references or existing copies. Keep the repository private until
the publication history and retained references have been addressed explicitly.

The source prerelease workflow accepts only alpha tags. It grants write access
only to the draft-creation job after the verification job succeeds. Regular CI
cannot publish binaries or releases. See `docs/RELEASE_NOTES.md` for draft copy.

## Binary releases

`scripts/build-release.ps1` and `scripts/build-redistributable.ps1` retain the
clean-source, Authenticode, checksum and manifest requirements. Do not convert
`-AllowUnsignedDevelopment` into a production-distribution bypass. Signing
credentials belong in protected release infrastructure, never in this repository.
The additional production requirements remain in `SECURITY_RELEASE_GATE.md`.

Source publication is not evidence that the trusted host is an OS sandbox or
that clean-VM, live-account, computer-control and SSH acceptance checks passed.
