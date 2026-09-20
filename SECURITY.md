# Security policy

## Supported versions

Only the current development branch and latest experimental source release
receive fixes. There is no stable, production-certified binary release yet.
See [the release gate](docs/SECURITY_RELEASE_GATE.md) for outstanding work and
[dependency security](docs/DEPENDENCY_SECURITY.md) for scoped exceptions.

## Reporting a vulnerability

Use GitHub's private **Report a vulnerability** flow on the repository's
[Security page](https://github.com/peppe311/supervisor/security/advisories/new).
If private reporting is unavailable, open an issue containing only a request
for a private contact channel. Do not publish an exploit, secret, private file
or sensitive reproduction in a public issue.

Include the affected commit/version, Windows version, provider/runtime version,
required permissions, impact and minimal reproduction using synthetic data.
Do not attach account stores, live credentials, complete chats or browser data.
There is no paid support SLA or bug-bounty promise.

## Security boundary

Supervisor runs on the user's machine. Its own terminal and SSH tools use the
authority of the configured local/remote account; approval UI is not a sandbox.
Codex's native tools use the sandbox and approval policy owned by App Server.
Full access can permit changes outside the selected project. Computer Use and
browser plugins have their own permissions and can interact with real sessions.

Treat pages, repository instructions, file contents and tool output as untrusted.
Keep credentials out of prompts and attachments, use trusted projects and review
requested capabilities. Closing the main window can leave agents running in the
tray; use Stop or Exit from the tray to end work.

Never disable path guards, account isolation, release signature checks or stop
mechanisms to make a test pass. Tests that require an authenticated provider,
remote server or desktop interaction must be explicitly opted into.
