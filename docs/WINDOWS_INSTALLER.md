# Windows installer

The installer is experimental and per user. It installs under
`%LOCALAPPDATA%\Programs\Supervisor`, creates a Start menu entry, offers an
optional desktop shortcut, and registers with Windows Installed Apps. It does
not start Supervisor automatically in silent mode, configure providers, bundle
credentials or copy an existing profile. Uninstall preserves chat history,
settings and project directories.

Close Supervisor from its notification-area menu before updating or uninstalling.
Both operations check the application's existing single-instance mutex. They do
not force-close agents, use Restart Manager or schedule replacement on reboot.
The current user's WebView2 Runtime must already be installed. If missing, setup
stops with Microsoft's official download address; it does not run an unverified
download. See [Microsoft's distribution guidance](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution).

## Build

Build the application from a clean checkout using `scripts/build-redistributable.ps1`.
Pass its five-file package to `scripts/build-installer.ps1`:

```powershell
./scripts/build-redistributable.ps1 -CertificateThumbprint <thumbprint>
./scripts/build-installer.ps1 -PackageDirectory ./outputs/Supervisor `
    -ReleaseTag v0.1.0-alpha.2 -CertificateThumbprint <thumbprint>
```

The application and installer must have the same valid signing certificate.
Inno Setup signs the uninstaller as well. The compiler is downloaded from the
official immutable Inno Setup 6.7.3 release, checked against its pinned SHA-256
and its valid Pyrsys B.V. Authenticode signature, and extracted in portable mode
inside `target/installer-tools/`. It adds no installed development application.
See [Inno Setup](https://jrsoftware.org/isinfo.php) and its license distributed
with the compiler. The generated installer retains Inno Setup's original credits.

Results go to `target/installer/`: installer EXE, SHA-256 checksum, and a manifest
containing the application source commit and signing mode. Only the five named
application files enter the payload. The working local package in `outputs/`
is not replaced by installer compilation.

For **local testing only**, pass `-AllowUnsignedDevelopment` instead of a
certificate to both build commands. Output explicitly includes `unsigned-local`
in its name. The production signing gate is unchanged. Do not upload this build
as a public release.

The maintainer explicitly authorized experimental unsigned alpha distribution on
2026-09-21. To exercise that narrow exception, run the installer builder from a
clean checkout with `-PublishUnsignedAlpha`, an `-alpha.N` release tag and a clean
unsigned application package. Stable releases still require signing. The installer
manifest records both the application's source commit and the installer recipe's
source commit; these may differ when packaging an unchanged published application.
The existing application package retains its original manifest and hashes.

## Validation and release

```powershell
./scripts/test-installer.ps1 -PackageDirectory ./outputs/Supervisor
```

The smoke test compiles the same recipe with a disposable application ID and
mutex. It checks fresh installation, exact payload hashes, replacement in place,
uninstall, active-agent blocking and preservation of unrelated synthetic content.
All files stay in a build-owned temporary directory; the real application is not
launched and its data is not accessed. This does not replace clean-VM testing of
missing prerequisites, Windows security prompts or the signed release path.

Before attaching binaries to a GitHub release, complete the applicable gates in
[SECURITY_RELEASE_GATE.md](SECURITY_RELEASE_GATE.md), verify signatures and hashes,
and link the download to the matching source tag. Never use a dirty local package
or artifacts from the private development archive. Publish the checksum and
installer manifest alongside the EXE. A website should enable its download only
after that exact release asset is available.
