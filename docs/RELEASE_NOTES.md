Supervisor's experimental open-source release is intended for Windows
developers and evaluators. It includes project and supervisor conversations,
official Codex App Server integration, browser/terminal/SSH tools, attachments,
live activity and diff presentation, and background execution controls.

## What's new

- **Project work review.** Project chat and Supervisor actions add Work summary
  from recorded turn evidence, Compare attempts through a read-only native
  Supervisor review, and Hand off to a native fork with a durable draft.
- **Project board.** Pinned projects, New task in a Git worktree, and Terminal
  and Git status actions from the project menu.
- **Estimated cache window.** The composer shows a compact countdown after a fresh
  native usage report that includes cached input.
- **Claude subscriptions.** The Claude Code status names the connected plan and
  warns when a sign-in bills per token. Usage-limit warnings, reached limits and
  extra usage appear with a relative reset time. Inherited API keys, alternative
  endpoints and third-party backends no longer reach the CLI, and resumed
  sessions no longer resend local history.

See [CHANGELOG.md](https://github.com/peppe311/supervisor/blob/main/CHANGELOG.md)
for earlier releases.

This release distributes source under MPL-2.0 with third-party exceptions and
notices. GitHub's source archives correspond to the release tag.

## Windows installer

The Windows x64 installer is an **unsigned experimental alpha**. Windows may show
an unknown-publisher warning. It installs for the current user, adds a Start menu
shortcut and supports Windows Installed Apps uninstall. Windows 10/11 and the
Microsoft Edge WebView2 Runtime are required. Provider runtimes, plugins and
accounts are configured separately. Close Supervisor through its tray icon before
updating; the installer never forcibly stops agents.

The installer was built from the clean tagged application source. Its accompanying
JSON identifies both application and installer-recipe commits. SHA-256 checksums
are attached for integrity checking; they do not replace a publisher signature.
Local validation passed full workspace checks, a hidden WebView startup check,
installation, in-place update, uninstall, active-app guards and preservation of
unrelated synthetic files. Clean-VM security-prompt and signed-path testing remain
outstanding. No chat data, account credentials or private project files are bundled.

No signed Windows installer or production-certified executable is included.
Desktop automation, authenticated providers, remote servers and account-specific
plugins require separate opt-in validation. Production signing and security gates
remain in force; only experimental unsigned alpha distribution was explicitly
authorized. Supervisor is independent of OpenAI and does not bundle official
desktop plugins, provider accounts or credentials.
