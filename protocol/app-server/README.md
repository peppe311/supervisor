# Official generated App Server contract

Supported runtimes: `codex-cli 0.153.4`, `codex-cli 0.154.0-alpha.6.2`,
`codex-cli 0.155.0-alpha.2.6` and `codex-cli 0.155.1`. The latest is the
official desktop executable installed and verified on 2026-09-20. App Server
does not have a separate release number.
`supported-versions.json` is the exact-version allowlist shared by runtime
discovery and the generated-schema verifier. Other versions remain unsupported
until their contracts and native behavior have been verified.

The versioned directories are generated directly by the matching official
executable:

```powershell
codex app-server generate-ts --out protocol/app-server/0.153.4/typescript
codex app-server generate-json-schema --out protocol/app-server/0.153.4/json
```

No experimental flag is used. Never hand-edit generated files. Regenerate into
a **new version directory**, review contract changes and run compatibility tests
before upgrading the supported runtime. Do not regenerate on every app launch.
The 2026-09-08 audit regenerated 304 JSON files and 706 TypeScript files with
zero path or content differences from the checked-in 0.153.4 directory.
The 2026-09-13 update generated a separate `0.154.0-alpha.6.2` directory with
the same commands and no experimental flag. No client or server request method
was removed. Existing serialized calls, approval decisions and incoming fixtures
pass against both versions. The version change adds optional originator, MCP
error and usage metadata, nullable usage-read parameters and generated definition
changes; it does not require a different handshake or account flow.
The 2026-09-17 update generated the separate `0.155.0-alpha.2.6` directory.
The stable contract is additive relative to `0.154.0-alpha.6.2`: it adds native
thread-attachment methods and a notification, removes no method, and keeps every
existing Supervisor request and fixture valid. Supervisor does not claim the new
attachment surface merely by accepting this runtime.
The 2026-09-20 update generated the separate stable `0.155.1` directory: 312
JSON schemas and 721 TypeScript files. Its stable contract is byte-for-byte
identical to `0.155.0-alpha.2.6`; all existing methods, typed adapters and
fixtures therefore retain the same wire shape. Runtime behavior is still
checked independently before the version is accepted.
Run `scripts/verify-app-server-contract.mjs` to check every supported version,
or pass one manifest version as its argument for a focused check.
See [OpenAI App Server documentation](https://learn.chatgpt.com/docs/app-server).
The repository also keeps
an [implementation guide](../../docs/CODEX_APP_SERVER_IMPLEMENTATION_GUIDE.md),
and a [method-level gap analysis](../../docs/CODEX_APP_SERVER_GAP_ANALYSIS.md).
Download private local reference documentation into ignored `target/reference-docs/`
with `scripts/sync-codex-app-server-docs.ps1`; regenerate protocol artifacts into a
new version directory when upgrading the CLI. Generated contracts retain the
upstream Apache-2.0 license; see `NOTICE` and `LICENSE-APACHE` in this directory.
