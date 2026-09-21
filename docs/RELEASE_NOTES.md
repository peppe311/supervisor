Supervisor's experimental open-source release is intended for Windows
developers and evaluators. It includes project and supervisor conversations,
official Codex App Server integration, browser/terminal/SSH tools, attachments,
live activity and diff presentation, and background execution controls.

This source prerelease replaces the initial publication with a new Git history.
Account-specific conversation metadata has been removed from engineering notes,
fixtures use synthetic identifiers, and export checks now detect native chat
identifiers and personal profile paths. The earlier repository is retained
privately; its commits, tags and pull-request references are not imported.

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
Local validation passed full workspace checks, hidden Light/Dark startup,
installation, in-place update, uninstall, active-app guards and preservation of
unrelated synthetic files. Clean-VM security-prompt and signed-path testing remain
outstanding. No chat data, account credentials or private project files are bundled.

No signed Windows installer or production-certified executable is included.
Desktop automation, authenticated providers, remote servers and account-specific
plugins require separate opt-in validation. Production signing and security gates
remain in force; only experimental unsigned alpha distribution was explicitly
authorized. Supervisor is independent of OpenAI and does not bundle official
desktop plugins, provider accounts or credentials.
