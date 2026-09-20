# Dependency security policy

Production release verification runs `cargo audit` against the locked dependency graph. An
advisory may be ignored only when its exact transitive path, reachable behavior, justification,
and removal condition are recorded here.

## RUSTSEC-2023-0071 — `rsa` timing side channel

- Transitive path: `central-agent -> ironrdp -> ironrdp-client/ironrdp-connector -> picky/sspi -> rsa`.
- Scope: Central Agent is an RDP client. It does not load, generate, or expose an RSA private key
  through this dependency graph; the reachable flow performs public certificate handling and
  client authentication through IronRDP/SSPI.
- Decision: temporarily ignored in the release audit because the advisory has no fixed upstream
  version and its vulnerable private-key operation is not used by Central Agent.
- Removal condition: remove the ignore immediately when IronRDP no longer resolves to the affected
  `rsa` implementation, or if Central Agent introduces certificate-based client authentication or
  any other private RSA operation in this graph.

Warnings for unmaintained target-specific GTK3 packages remain visible in audit output. They are
not part of the current Windows binary, but must be resolved before claiming a supported Linux
release.
