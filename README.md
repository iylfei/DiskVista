# DiskVista

[简体中文](README.zh-CN.md)

DiskVista is a disk space analyzer and cleanup tool for Windows 10/11 x64.

## Features

- Scan a disk or folder and inspect file sizes and space usage.
- Find large files with a space map and navigate folders in place.
- Review usage by application, browse its files without leaving Application Space, and return to the previous list position.
- Browse cleanup suggestions such as temporary files and caches, together with their possible impact.
- Select files manually, review a safety preview, and move approved items to the Windows Recycle Bin.
- Review scan records and operation results, protect paths, or explicitly remove protection from a selected path after confirmation.
- Optionally use AI-assisted explanations for unclear files. AI is disabled by default.
- Switch the complete application interface between Simplified Chinese and English in Settings.

## Usage

Run the installer for the installed edition. For the portable edition, extract the entire directory and run `diskvista.exe`; keep `diskvista-worker.exe` and the other bundled files beside it. DiskVista requires the Windows WebView2 Runtime.

1. Select a disk or folder and wait for scanning to finish.
2. Browse files from Cleanup Suggestions, Application Space, or Space Map.
3. Add content you no longer need to the cleanup list, review the preview, and confirm recycling.

Scanning never deletes files. Cleanup only moves approved files to the Recycle Bin; it does not permanently delete them or empty the Recycle Bin automatically, so disk space is usually not released immediately. System protection remains enabled by default, scan roots cannot be unprotected, and every protection override requires explicit confirmation.

Local scanning and cleanup do not require an internet connection. AI features require a compatible API configured by the user and request consent before sending information.

## Build

Rust MSVC, the Windows C++ build tools, Node.js, and pnpm are required.

```powershell
pnpm install --frozen-lockfile
pnpm desktop:dev
```

Run `pnpm desktop:build` to create the installer and portable package in `artifacts/`.

## Third-party projects

- [Tauri](https://github.com/tauri-apps/tauri): desktop application framework.
- [React](https://github.com/facebook/react): user-interface components.
- [TanStack Virtual](https://github.com/TanStack/virtual): virtualized long lists.
- [Lucide](https://github.com/lucide-icons/lucide): interface icons.
- [Winapp2](https://github.com/MoscaDotTo/Winapp2): data source for optional community rules. DiskVista converts a supported subset of its file-cleanup rules, retains the CC BY-SA 4.0 license, and does not include Winapp3.

Development also referenced [Bulk Crap Uninstaller](https://github.com/BCUninstaller/Bulk-Crap-Uninstaller), [Czkawka](https://github.com/qarmin/czkawka), and [sdirstat](https://github.com/Ptyktos/sdirstat). They are not included as code dependencies.

See the [third-party notice](third-party/NOTICE.md) for sources, rule modifications, and license details.

## License

Original code in this repository is licensed under [PolyForm Noncommercial 1.0.0](https://polyformproject.org/licenses/noncommercial/1.0.0). Noncommercial use, modification, and distribution must follow that license; commercial use requires separate permission. See [LICENSE](LICENSE) for the complete text.

Third-party components retain their own licenses and are not subject to this project's noncommercial restriction. Winapp2-derived rules in `assets/rules/community.json` remain licensed under CC BY-SA 4.0.
