# Third-party notices

## Winapp2 rule data

- Upstream: https://github.com/MoscaDotTo/Winapp2
- Pinned commit: `b8fa0bd9cc5e59f17a34fe71c5464c7450180a37`
- Original author/community: MoscaDotTo and Winapp2 contributors.
- License: Creative Commons Attribution-ShareAlike 4.0 International; original terms are in `winapp2/License.md`.
- Modified here: `scripts/vendor-rules.mjs` translates a deliberately narrow subset of file-only targets into `assets/rules/community.json`, preserving warnings and provenance. Unsupported entries are skipped in full. No registry changes or scripts are imported. Winapp3 is not included. The adapted rule data retains CC BY-SA 4.0.
- The bundled original `Winapp2.ini` and derived JSON are data, never executable cleanup code.

## Other dependencies

`dependencies/inventory.json` records exact installed crate/npm versions and declared licenses; their original texts are beside the inventory. This over-inclusive inventory also lists build/test tools that are not linked into the application. A shared repository license is reused only when the declared license expression agrees. Fixed upstream commits are recorded where crates omit license files. Exact unmodified MPL crate sources are retained in `source-dist` with their original attribution.

The development-only npm package `stackback@0.0.2` declares MIT and author Roman Shtylman in its package manifest but does not ship a license text. It is not bundled into the desktop runtime. Upstream package: https://www.npmjs.com/package/stackback/v/0.0.2 ; repository inspected at `24d55717e16f1bb2109e848ea2b3a90ea900ddb5`.

## Design references (no source code copied)

- Bulk Crap Uninstaller: application inventory and ownership evidence; https://github.com/BCUninstaller/Bulk-Crap-Uninstaller
- Czkawka Core: bounded/cancellable scan and identity design; https://github.com/qarmin/czkawka/tree/master/czkawka_core
- sdirstat: space aggregation and visualization; https://github.com/Ptyktos/sdirstat

No cleanup implementation from these projects is executed or embedded. Project-specific source licensing remains separate from all third-party terms.
