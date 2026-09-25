# Changelog

## 0.1.0-alpha.4 — project work review and Claude subscriptions

- Project chat and Supervisor actions: Work summary from recorded turn evidence,
  Compare attempts through a read-only native Supervisor review, and Hand off
  to a native fork with a durable draft for the next instruction.
- Pinned projects, New task in a Git worktree, and project-menu Terminal and Git
  status actions from the project board.
- Compact estimated cache window in the composer after a fresh native usage
  report that includes cached input.
- Claude Code: the provider status names the connected Claude plan and warns
  when a sign-in bills per token. Subscription usage-limit warnings, reached
  limits and extra usage appear with a relative reset time. Inherited API keys,
  alternative endpoints and third-party backends no longer reach the CLI, and
  resumed sessions no longer resend local history.
- Binary privacy gate for release artifacts and the installer pipeline.

## 0.1.0-alpha.3 — Windows installer

- Per-user Windows installer for experimental unsigned alpha releases, with
  Start menu entry, Installed Apps uninstall and preserved user data.

## 0.1.0-alpha.2 — corrected source publication

- Fresh publication history, separate from the privately retained first attempt.
- Removed account-specific chat metadata from historical documentation.
- Synthetic identifiers and profile paths in fixtures; publication privacy checks
  reject native conversation IDs, personal paths and runtime data exports.

## 0.1.0-alpha.1 — initial experimental source release

- Windows desktop workspace built with Rust, Svelte and TypeScript.
- Project conversations, supervisor conversations and associated project files.
- Official Codex App Server integration, streaming activity, approvals, steering,
  interruption/resume, context usage, history and conversation forks.
- Local project onboarding, Git information, SSH connections, terminal and browser.
- Plugin selection and integrations with separately installed provider tools.
- Background work with Windows tray controls and explicit stop on exit.
- MPL-2.0 licensing, dependency notices, contributor/security documentation and
  automated Windows verification.

This is a source release for development and evaluation. Provider availability
depends on separately installed runtimes and account capabilities. A passing CI
run does not certify desktop automation, provider accounts or binary signing.
