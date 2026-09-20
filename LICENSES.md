# Licensing

Supervisor's original source code, scripts and documentation are licensed under
the [Mozilla Public License 2.0](LICENSE). Original brand artwork is included
under the same license; trademark rights are not granted. The repository-wide
notice is in [NOTICE](NOTICE); file groups are recorded in [REUSE.toml](REUSE.toml).

MPL is a file-level copyleft license. You may use, modify and distribute
Supervisor, including commercially. When you distribute modified covered files,
make their corresponding source available under MPL 2.0 and retain notices.
For the actual conditions, use the license text and
[Mozilla's FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/).

## Third-party exceptions

| Material | License and provenance |
| --- | --- |
| `vendor/ironrdp-client` | MIT OR Apache-2.0; upstream license files and local patch notes are retained. |
| `vendor/vnc-rs` | MIT OR Apache-2.0; upstream license files and local patch notes are retained. |
| `protocol/app-server/*` generated contracts | Apache-2.0; generated from OpenAI Codex, with attribution in `protocol/app-server/NOTICE`. |
| `assets/fonts/inter`, `assets/fonts/manrope` | SIL OFL 1.1; each directory contains its copyright, license and source/modification notes. |
| Selected Simple Icons SVG paths | CC0-1.0; third-party marks remain their owners' trademarks. |
| Rust and npm dependencies, including compiled frontend code | Their own licenses; see `THIRD_PARTY_NOTICES.md` and `licenses/dependencies.json`. |

Apple SF Pro is not bundled. The UI can use an available system font and ships
OFL fonts as fallbacks. Official Codex plugins and account data are not bundled.
Copied OpenAI web documentation is excluded from the publication; our own
implementation studies link to the official documentation instead.

## Updating dependencies

Run `scripts/update-licenses.ps1` after changing either lockfile. This collects
the available upstream license, copyright and notice files, keeps pinned
fallbacks for packages that omit them, and regenerates the notices and inventory.
Run `scripts/update-licenses.ps1 -Check` to detect drift without writing files.
The supported notice-generation platform is Windows x64 with Node 24.

The inventory covers the complete locked Cargo graph, including dependencies
for other targets, and the locked npm graph. Optional npm binaries for other
platforms are identified separately and are not shipped in the Windows app.
Automated coverage is not a certification of legal compliance. Review new
licenses, bundled code and source-offer requirements before distributing a binary.

## Binary distribution

Keep `LICENSE.txt`, `THIRD_PARTY_NOTICES.md`, the dependency inventory and release
manifest beside the executable. Publish the matching source tag and any local
patches. The first open-source release is an experimental source release;
production binaries remain subject to the existing signing and security gates.
