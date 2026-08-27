# DiskVista

适用于 Windows 10/11 x64 的磁盘空间分析与清理工具。

## 功能

- 扫描磁盘或文件夹，查看文件大小和占用分布。
- 通过空间地图查找大文件，点击路径返回上级目录。
- 按应用查看占用，展开查看内部组件。
- 按类别查看临时文件、缓存等清理建议，以及清理可能带来的影响。
- 手动选择文件，检查预览后移入 Windows 回收站。
- 查看扫描记录、处理结果，并保护不希望清理的路径。
- 可选 AI 辅助分析文件用途，默认关闭。

## 使用

安装版运行安装程序。免安装版解压整个目录后运行 `diskvista.exe`，保留同目录的 `diskvista-worker.exe` 和其他附带文件。程序需要 Windows WebView2 运行时。

1. 选择磁盘或文件夹，等待扫描完成。
2. 从清理建议、应用空间或空间地图中查看文件。
3. 将不再需要的内容加入待清理清单，检查预览后确认回收。

扫描不会删除文件。清理只移入回收站，不会永久删除或自动清空回收站，因此通常不会立即释放磁盘空间。受保护的系统文件和应用不能直接清理。

本地扫描和清理不需要联网。AI 功能需要自行配置兼容 API，发送信息前会请求授权。

## 构建

需要 Rust MSVC、Windows C++ 构建工具、Node.js 和 pnpm。

```powershell
pnpm install --frozen-lockfile
pnpm desktop:dev
```

运行 `pnpm desktop:build` 生成安装包和免安装包，输出到 `artifacts/`。

## 第三方项目

- [Tauri](https://github.com/tauri-apps/tauri)：桌面应用框架。
- [React](https://github.com/facebook/react)：界面组件。
- [TanStack Virtual](https://github.com/TanStack/virtual)：长列表显示。
- [Lucide](https://github.com/lucide-icons/lucide)：界面图标。
- [Winapp2](https://github.com/MoscaDotTo/Winapp2)：可选社区规则的数据来源。本项目转换了其中部分文件清理规则，保留 CC BY-SA 4.0 许可，不包含 Winapp3。

开发时还参考了 [Bulk Crap Uninstaller](https://github.com/BCUninstaller/Bulk-Crap-Uninstaller)、[Czkawka](https://github.com/qarmin/czkawka) 和 [sdirstat](https://github.com/Ptyktos/sdirstat)，未将其作为代码依赖引入。

来源、规则修改说明和许可信息见 [第三方声明](third-party/NOTICE.md)。

## 许可

本项目自有代码采用 [PolyForm Noncommercial 1.0.0](https://polyformproject.org/licenses/noncommercial/1.0.0)。非商业使用、修改和分发须遵守该许可证，商业用途需另行授权。完整文本见 [LICENSE](LICENSE)。

第三方组件保留各自许可证，不适用本项目的非商业限制。Winapp2 衍生规则（`assets/rules/community.json`）仍采用 CC BY-SA 4.0。
