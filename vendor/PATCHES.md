# Vendored changes

These packages retain their upstream MIT OR Apache-2.0 licenses. Supervisor's
MPL license does not replace them. The exact published upstream crate versions
and checksums are recorded in the Cargo manifests/lockfile and `.cargo_vcs_info.json`.

- `ironrdp-client` 0.1.0: preserve the dirty rectangle instead of copying the
  complete framebuffer for every graphics update. Upstream revision:
  `11a0810cfbbabd8b8023875a05e3041216d4b01b` in Devolutions/IronRDP.
- `vnc-rs` 0.5.3: expose ExtendedDesktopSize to request the fixed VNC desktop.

See the comments in the root `[patch.crates-io]` section. Keep patches small,
document future changes here and remove a patch when the upstream API supports
the required behavior. Both license alternatives and bundled notices must be
preserved when redistributing these sources.
