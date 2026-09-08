const exactEnglish: Record<string, string> = {
  等待开始检查: "Waiting to check",
  准备检查: "Preparing checks",
  读取扫描记录: "Reading scan records",
  核对当前文件: "Checking current files",
  核对保护状态: "Checking protections",
  检查文件占用: "Checking files in use",
  核对可回收大小: "Checking recyclable size",
  安全检查完成: "Safety checks complete",
  安全检查已取消: "Safety checks cancelled",
  安全检查未完成: "Safety checks did not complete",
  "正在取消安全检查…": "Cancelling safety checks…",
  本阶段已检查: "Checked in this stage:",
  "取消检查失败：": "Could not cancel checks: ",
  "。请重试关闭。": ". Please try closing again.",
  取消并关闭: "Cancel and close",
  "· 正在统计占用…": "· Calculating usage…",
  "· 正在载入扫描结果…": "· Loading scan results…",
  "无法读取列表，请重试。": "Unable to load the list. Please retry.",
  "。回收仍在进行，可重试取消剩余项。":
    ". Recycling is still in progress. You can retry cancelling the remaining items.",
  "· 保持不发送长度参数": "· Keep output length parameter disabled",
  "· 参考 {1} 条回收历史": "· {1} recycling history references",
  "· 目录": "· Folder",
  "· 文件占用约": "· File usage about",
  "· 已展开子项": "· Components expanded",
  "· 占用约 {1}": "· About {1}",
  "· 至少": "· At least",
  "（估算）": " (estimated)",
  "（仅供参考）": " (for reference only)",
  "（无法精确计算）": " (cannot be calculated precisely)",
  "{1} {2}，{3}": "{1} {2}, {3}",
  "{1}，打开 AI 设置": "{1}. Open AI settings",
  "{1}；仅供参考，点击查看完整分析":
    "{1}; for reference only. Click to view the full analysis",
  "{1}的操作": "Actions for {1}",
  "{1}的用途与清理影响": "Purpose and cleanup impact of {1}",
  安装包: "Installer package",
  安装文件: "Installation files",
  按大小: "By size",
  "按类别查找文件，勾选后添加到待清理清单。":
    "Browse files by category, select them, and add them to the cleanup list.",
  按路径: "By path",
  "按目录分批分析来源未明确的大文件，文件须严格大于门槛（最低 100 MiB）；每项提供删除建议与简短理由。":
    "Analyze large files with unclear origins in folder-based batches. Files must be strictly larger than the threshold (minimum 100 MiB); each receives deletion advice and a short reason.",
  "按文件内容计算的大小，文件夹中是所含文件的合计。它可能与磁盘实际占用不同，也不代表可以释放多少空间。":
    "Size calculated from file contents; folders show the total of contained files. It may differ from actual disk usage and does not indicate how much space can be freed.",
  按占用空间: "By space used",
  "包含多个文件位置，请展开后分别添加":
    "Contains multiple file locations. Expand and add them separately",
  保存: "Save",
  保存保护设置: "Save protection settings",
  "保存后，下次启动时移除较早的扫描记录；增量扫描需要的记录会额外保留。不删除原文件，也不移除回收操作历史。":
    "After saving, older scan records are removed on the next launch. Records needed for incremental scans are retained separately. Original files and recycling history are not deleted.",
  保存设置: "Save settings",
  保护此路径: "Protect this path",
  保护设置已保存: "Protection settings saved",
  保留全部: "Keep all",
  本次处理结果: "Results of this operation",
  "本次扫描没有完成。请选择已完成的扫描记录，或重新扫描。":
    "This scan did not finish. Select a completed scan record or scan again.",
  本地磁盘: "Local disks",
  本机免密服务可留空: "Leave blank for a local service without authentication",
  "本批 {1} 项合计 ·": "Batch total for {1} items ·",
  "表示判断来源的依据有多充分：高表示有明确记录，中表示有相关线索，低表示仍需确认。它不代表删除风险，认出所属应用也不等于可以删除。":
    "Indicates how strong the evidence is for the identified origin: High means clear records, Medium means related clues, and Low still requires verification. It is not deletion risk, and identifying an application does not mean the item can be deleted.",
  不发送: "Do not send",
  不发送长度参数: "Do not send a length parameter",
  "不使用 AI 也能正常清理": "Cleanup works without AI",
  "不同服务支持的回答格式不同，通常保留“自动”即可。JSON 是软件便于读取的数据格式，JSON Schema 会进一步约束回答结构；无论选哪种，软件都会检查结果。":
    "Services support different response formats; Auto is usually best. JSON is a machine-readable data format, while JSON Schema constrains the response structure further. DiskVista validates the result in every mode.",
  不完整: "Incomplete",
  不想清理或发送的内容: "Items you do not want to clean or send",
  "部分位置可能因权限不足、设备断开或读取失败而没有扫描完整。未读到不等于没有文件；相关大小可能偏小，不能据此整体清理。":
    "Some locations may be incomplete because of insufficient permissions, a disconnected device, or a read failure. Unread does not mean empty; related sizes may be understated and must not be used to clean the whole location.",
  "参考最大输出：": "Reference maximum output:",
  操作历史: "Operation history",
  操作历史分页: "Operation history pagination",
  操作历史加载失败: "Failed to load operation history",
  测试连接: "Test connection",
  "曾移入回收站不代表之后没有还原，也不代表相似文件可以安全删除。":
    "A previously recycled item may have been restored, and similar files are not necessarily safe to delete.",
  "查看 {1}": "View {1}",
  "查看 AI 分析：{1}": "View AI analysis: {1}",
  查看结果: "View results",
  查看下载中的大文件: "Find large files in Downloads",
  查看空间地图: "View space map",
  "查看每个文件的处理结果。": "View the result for each file.",
  查看全部文件: "View all files",
  查看文件: "View files",
  "查看文件 {1}": "View file {1}",
  查看文件列表: "View file list",
  查看详情: "View details",
  "查看详情 {1}": "View details for {1}",
  查看原因: "View reason",
  "尝试读取 Windows 已有的文件变更记录（USN Journal），辅助后续扫描。可能需要你确认管理员权限；不修改系统记录，无法使用时会改为完整扫描，不保留后台服务。":
    "Try to read the existing Windows file change journal (USN Journal) to assist later scans. Administrator approval may be required. DiskVista does not modify the journal, falls back to a full scan when unavailable, and does not leave a background service running.",
  程序文件: "Program files",
  程序组件: "Program components",
  处未能扫描: " locations could not be scanned",
  窗口控制: "Window controls",
  磁盘镜像: "Disk image",
  "此处是未归属内容的统计，请查看文件后选择具体项目":
    "This is a summary of unassigned content. View the files and select specific items",
  "此项目受保护，不能回收": "This item is protected and cannot be recycled",
  "此项目受保护或未扫描完整，请查看详情":
    "This item is protected or was not scanned completely. View details",
  "此项目未扫描完整，请重新扫描后再操作":
    "This item was not scanned completely. Scan again before continuing",
  "从清理建议中添加文件。": "Add files from Cleanup Suggestions.",
  "从最近成功移入回收站的记录中，为每个文件挑选最多 6 条应用、目录或命名线索相关的参考。历史不会扩大批量候选范围，也不会自动勾选、降低保护或证明文件可删。":
    "Select up to six related application, folder, or naming clues for each file from recent successful recycling history. History never expands the batch candidate scope, selects items automatically, lowers protection, or proves that a file is safe to delete.",
  "打开 {1}": "Open {1}",
  "打开 Windows 回收站": "Open Windows Recycle Bin",
  打开文件夹: "Open folder",
  "大小、来源与时间": "Size, origin, and time",
  待清理: "Pending cleanup",
  待清理清单: "Cleanup list",
  "待清理清单：{1} 项": "Cleanup list: {1} items",
  待清理清单为空: "Cleanup list is empty",
  "单次分析请求的生成预算，不是输入或上下文窗口大小。请单独确认服务和模型支持的最大输出，1M 上下文不等于 1M 输出。测试连接使用固定的轻量预算，不能验证这里设置的完整上限。":
    "Generation budget for one analysis request, not the input or context-window size. Confirm the maximum output supported by the service and model separately; a 1M context window is not 1M output. The connection test uses a fixed lightweight budget and cannot validate the full limit configured here.",
  单次输出上限: "Output limit per request",
  "单次输出上限（token）": "Output limit per request (tokens)",
  "单次输出上限必须是 1 到 1,048,576 之间的整数。":
    "The output limit must be an integer from 1 to 1,048,576.",
  "当前不发送此数值。": "This value is not currently sent. ",
  当前目录: "Current folder",
  当前目录的文件: "Files in the current folder",
  "当前目录空间地图，按文件大小绘制":
    "Space map of the current folder, drawn by file size",
  当前目录没有文件: "The current folder has no files",
  当前扫描: "Current scan",
  当前识别规则: "Current identification rules",
  "当前文件，或这个文件夹内所有已扫描文件中，最近一次修改的时间。它能提示内容有过变化，但不能证明应用最近被使用过。":
    "The most recent modification time of this file, or of all scanned files in this folder. It can indicate that content changed, but does not prove that an application was used recently.",
  当前文件夹路径: "Current folder path",
  当前页码: "Current page",
  当时参考的回收历史: "Recycling history referenced at that time",
  地图中的文件大小: "File sizes on the map",
  "等待 AI 服务回复的最长时间。超时只会停止当前分析，不影响本地扫描，也不会操作文件。":
    "Maximum time to wait for the AI service. A timeout only stops the current analysis; it does not affect local scanning or operate on files.",
  低: "Low",
  "点击上方的磁盘，或选择一个文件夹开始扫描。":
    "Click a disk above or choose a folder to start scanning.",
  "点击应用可展开安装与数据位置；未归属的内容单独列出。大小只包含本次扫描范围，已归入应用的部分不会重复计入其他目录。识别结果仅供参考，应用组件不能据此直接删除。":
    "Click an application to expand its installation and data locations. Unassigned content is listed separately. Sizes cover only the current scan, and content assigned to an application is not counted again under other folders. Identification is for reference only and does not make application components safe to delete.",
  "读取的项目不一致，请重新选择":
    "The loaded items do not match. Select them again",
  "读取设置与扫描记录，不会自动扫描磁盘。":
    "Loading settings and scan records. This does not scan disks automatically.",
  "多应用集合（非单个应用）":
    "Multiple-application group (not one application)",
  返回: "Back",
  返回分类: "Back to categories",
  "方块越大，文件越大。点击文件夹继续查看。":
    "Larger tiles represent larger files. Click a folder to continue browsing.",
  分: "points",
  分析参考信息: "Analysis reference information",
  "分析结果可在对应文件详情的“AI 辅助解释”中查看。":
    "Analysis results are available under AI-assisted explanation in the corresponding file details.",
  "分析条件已变化，请重新分析。":
    "Analysis conditions have changed. Analyze again.",
  分析完成: "Analysis complete",
  "分析位置：": "Analysis location:",
  "分析已过期，打开详情查看原因":
    "Analysis has expired. Open details to see why",
  风险筛选: "Risk filter",
  高: "High",
  个文件: " files",
  "个文件 ·": " files ·",
  个文件或文件夹: " files or folders",
  "根据已知应用常见的文件位置和用途提供判断依据，不是自动删除指令。所有清理仍需你确认，只有你可以在文件详情中解除保护。":
    "Provides evidence based on common file locations and uses of known applications. It is not an automatic deletion instruction. Every cleanup still requires your confirmation, and only you can remove protection in file details.",
  "更改地址会清除旧密钥，请重新填写。":
    "Changing the address clears the old key. Enter it again.",
  功能: "Features",
  共: "Total",
  "共 {1} 条 ·": "{1} total ·",
  估算: "Estimated",
  固定磁盘: "Fixed disks",
  关闭: "Close",
  "关闭 AI 分析提示": "Close AI analysis message",
  关闭窗口: "Close window",
  关闭对话框: "Close dialog",
  关闭提示: "Close message",
  关闭详情: "Close details",
  关联应用: "Related application",
  "关于{1}": "About {1}",
  归属置信度: "Origin confidence",
  "规则和 AI 只能给出建议；只有你可以在文件详情中解除保护。":
    "Rules and AI can only offer advice; only you can remove protection in file details.",
  规则中心: "Rule Center",
  还没有扫描记录: "No scan records yet",
  还原窗口: "Restore window",
  含子项: "Includes components",
  合计: "Total",
  "后台仍会检查文件变化、保护状态、占用和回收站配置。文件只会移入 Windows 回收站，不会永久删除。":
    "DiskVista still checks file changes, protection status, usage, and Recycle Bin configuration. Files are moved only to the Windows Recycle Bin and are never permanently deleted.",
  忽略此项目: "Ignore this item",
  忽略的项目: "Ignored items",
  缓存: "Cache",
  "恢复：": "Recovery:",
  "回收检查与当前清单不一致，请重试。":
    "The recycling check no longer matches the current list. Try again.",
  回收历史参考: "Recycling history references",
  基于扫描记录: "Based on scan records",
  检查临时文件: "Check temporary files",
  加入待清理清单: "Add to cleanup list",
  "加入待清理清单 {1}": "Add {1} to cleanup list",
  "兼容 API 地址": "Compatible API address",
  建议保留: "Keep recommended",
  建议排序: "Suggestion sorting",
  "将删除本次扫描的文件索引和 AI 分析结果，无法撤销。原文件和回收操作历史不会删除。":
    "This permanently deletes the file index and AI analysis results for this scan. Original files and recycling history are not deleted.",
  较低风险: "Lower risk",
  "较低风险表示有依据认为内容通常可重新生成，仍需你确认。需要你确认表示用途或影响尚不明确；受保护内容不能加入待清理清单。":
    "Lower risk means there is evidence that the content is usually reproducible, but your confirmation is still required. Needs review means its purpose or impact remains unclear. Protected content cannot be added to the cleanup list.",
  "接口；当前需要兼容 chat/completions 的服务地址。":
    " endpoint; a service address compatible with chat/completions is currently required.",
  解除文件受保护状态: "Remove file protection",
  "解除文件受保护状态？": "Remove file protection?",
  "仅按模型名称匹配参考，当前服务的实际限制可能不同。":
    "Reference matched only by model name. The current service may have different limits.",
  "仅从本软件近 180 天成功回收的记录中挑选，每个文件最多 6 条。历史只作参考，不扩大分析范围。会发送脱敏路径、大小、回收时间及已有的应用归属，不包含文件正文；失败或取消的操作不参与。关闭后不再发送，更改服务地址后需重新开启。":
    "Select up to six successful recycling records from the last 180 days for each file. History is for reference only and does not expand the analysis scope. Redacted paths, sizes, recycling times, and known application ownership are sent without file contents; failed or cancelled operations are excluded. Turning this off stops sending, and changing the service address requires new consent.",
  "仅发送固定轻量测试消息（128 token 输出预算），不发送文件信息；不会验证单次输出上限":
    "Sends only a fixed lightweight test message (128-token output budget), with no file information; does not validate the configured output limit",
  仅建议: "Advice only",
  仅看受保护项: "Protected items only",
  "禁止发送 AI": "Exclude from AI",
  "禁止发送 AI 的路径": "Paths excluded from AI",
  可考虑删除: "Consider deleting",
  可能的应用目录: "Possible application folder",
  "可能关联：": "Possible association:",
  "可选 AI 分析": "Optional AI analysis",
  "可选：读取少量文件内容": "Optional: read limited file content",
  "可以调整 AI 分析筛选，或清除筛选查看其他文件。":
    "Adjust the AI analysis filter or clear it to view other files.",
  "可以返回上一级，或调整筛选条件。": "Go up one level or adjust the filters.",
  "可以返回上一级，或调整搜索条件。": "Go up one level or adjust the search.",
  "可以清除筛选，查看其他清理建议。":
    "Clear the filters to view other cleanup suggestions.",
  "可以在空间地图查看占用分布，或扫描下载、临时文件等其他位置。":
    "View usage distribution in Space Map, or scan other locations such as Downloads and temporary files.",
  "可以重新扫描，或在空间地图中查看已经扫描到的内容。":
    "Scan again or view the content already found in Space Map.",
  可用: "Available",
  "可在 Windows 回收站中选择“还原”恢复文件。":
    "Restore files from the Windows Recycle Bin.",
  空间地图: "Space Map",
  "空间地图与当前目录不一致，请重新读取":
    "The space map no longer matches the current folder. Reload it",
  空间分析与清理: "Space analysis and cleanup",
  来源: "Origin",
  "来源是根据现有线索判断的，不一定是最初创建文件的程序。“很久没访问”也不代表可以删除。":
    "Origin is inferred from available clues and may not be the program that originally created the file. A file not accessed for a long time is not necessarily safe to delete.",
  "来自 Winapp2 的额外识别规则，随软件提供、默认关闭。只采用能完整理解的规则，含不支持的检测、排除或警告时会整条跳过。":
    "Additional identification rules from Winapp2, included with DiskVista and disabled by default. Only fully understood rules are used; a rule is skipped entirely if it contains unsupported detection, exclusion, or warning logic.",
  "类型 / 所属应用": "Type / application",
  临时文件: "Temporary files",
  逻辑大小: "Logical size",
  没有发现可供选择的清理建议: "No selectable cleanup suggestions found",
  "没有符合当前 AI 筛选的文件": "No files match the current AI filter",
  没有符合当前筛选的项目: "No items match the current filters",
  没有符合条件的文件: "No files match the conditions",
  没有可操作的文件位置: "No actionable file locations",
  "没有项目通过安全检查{1}": "No items passed the safety check{1}",
  没有找到较低风险的项目: "No lower-risk items found",
  没有找到匹配的文件: "No matching files found",
  没有找到匹配的应用或文件夹: "No matching applications or folders found",
  每次扫描请求上限: "Request limit per scan",
  "每次只能处理 1–{1} 个不重复的项目。":
    "Each operation can process 1–{1} unique items.",
  "每次最多选择 {1} 项。请先处理当前已选内容，或减少选择。":
    "You can select up to {1} items at a time. Process the current selection or select fewer items.",
  "每次最多选择 500 项，请缩小搜索范围或按页选择":
    "You can select up to 500 items at a time. Narrow the search or select one page at a time",
  每页: "Per page",
  "密钥已从 Windows 凭据存储移除":
    "The key was removed from Windows Credential Manager",
  "秒 ·": " sec ·",
  "名称 / 路径": "Name / path",
  "模型 ID": "Model ID",
  模型参考信息: "Model reference information",
  "默认不开启 AI，不联网也可以扫描与回收。":
    "AI is disabled by default. Scanning and recycling work without an internet connection.",
  "默认关闭，可帮助识别更多应用文件。仅采用软件能安全理解的规则；切换后请重新扫描。":
    "Disabled by default. It can identify more application files. Only rules DiskVista can safely understand are used; scan again after changing this option.",
  "默认关闭，可能需要管理员许可。无法使用时会自动改为完整扫描，不修改系统设置。":
    "Disabled by default and may require administrator approval. DiskVista automatically falls back to a full scan when unavailable and does not change system settings.",
  默认开启: "Enabled by default",
  "默认只发送部分文件名、文件夹结构、大小、时间和判断依据，不包含文件正文。用户名会被替换，但文件名仍可能透露隐私。读取正文需要你另行同意，密码、钱包等敏感文件不允许读取正文。":
    "By default, only selected file names, folder structure, sizes, times, and evidence are sent, never file contents. User names are replaced, but file names may still reveal private information. Reading file contents requires separate consent, and sensitive files such as passwords and wallets are never sampled.",
  目录记录为: "Folder recorded as",
  目录类型: "Folder type",
  目录内包含受保护或未完整扫描的内容:
    "The folder contains protected or incompletely scanned content",
  "目录内容还在读取，请等待扫描完成。":
    "Folder contents are still being read. Wait for the scan to finish.",
  "目录已将此型号标为弃用，请确认当前服务仍然提供。":
    "The catalog marks this model as deprecated. Confirm that the current service still offers it.",
  "哪些信息会发送给 AI": "Information sent to AI",
  内容最新变化: "Latest content change",
  内置规则: "Built-in rules",
  排序: "Sort",
  判断依据与注意事项: "Evidence and notes",
  "批量只分析来源未明确、严格大于门槛的文件，不包含目录或已明确所属应用的文件。门槛最低按 100 MiB 生效，也可调高；旧设置低于 100 MiB 时仍使用此最低门槛。按目录分批发送，每批最多 20 个文件。":
    "Batch analysis includes only files with unclear origins that are strictly larger than the threshold. Folders and files already linked to applications are excluded. The minimum effective threshold is 100 MiB and can be raised; older lower settings still use this minimum. Files are sent in folder-based batches of up to 20.",
  其他扫描记录: "Another scan record",
  "其余 {1} 项": "Other {1} items",
  "启用 AI": "Enable AI",
  "启用社区规则（Winapp2）": "Enable community rules (Winapp2)",
  "启用增强扫描（USN）": "Enable enhanced scanning (USN)",
  起始建议: "Starting suggestions",
  清除筛选: "Clear filters",
  清空清单: "Clear list",
  清理风险: "Cleanup risk",
  清理建议: "Cleanup Suggestions",
  清理建议筛选: "Cleanup suggestion filters",
  清理前须知: "Before cleanup",
  清理需要你确认: "Cleanup requires your confirmation",
  "清理影响：": "Cleanup impact:",
  请求超时: "Request timeout",
  "请求超时（秒）": "Request timeout (seconds)",
  请求上限: "Request limit",
  "请稍后重试。": "Try again later.",
  "请使用 Windows 桌面程序，普通浏览器不能访问本地磁盘。":
    "Use the Windows desktop application. A regular browser cannot access local disks.",
  "请先等待扫描、AI 分析或回收结束":
    "Wait for scanning, AI analysis, or recycling to finish",
  "请先等待扫描、AI 分析或回收结束。":
    "Wait for scanning, AI analysis, or recycling to finish.",
  "请先关闭相关应用。每次最多选择 500 项。":
    "Close related applications first. Up to 500 items can be selected at a time.",
  "请先扫描一个位置。": "Scan a location first.",
  请先完成或取消当前扫描: "Finish or cancel the current scan first",
  "请先完成扫描，再分析扫描结果。":
    "Finish the scan before analyzing its results.",
  "请先完成扫描，再加入待清理清单":
    "Finish the scan before adding items to the cleanup list",
  "请运行 Windows 桌面程序；普通浏览器不具备磁盘访问权限。":
    "Run the Windows desktop application; a regular browser does not have disk access.",
  "请在设置中填写服务地址和模型。":
    "Enter a service address and model in Settings.",
  取消: "Cancel",
  取消分析: "Cancel analysis",
  "取消请求失败：": "Cancellation request failed:",
  取消扫描: "Cancel scan",
  取消剩余项: "Cancel remaining items",
  取消选择: "Clear selection",
  去查看清理建议: "View cleanup suggestions",
  "全部 AI 状态": "All AI statuses",
  "全部建议（不含受保护）": "All suggestions (excluding protected items)",
  全部文件: "All files",
  "确定删除这条扫描记录吗？": "Delete this scan record?",
  确认发送并分析: "Confirm and analyze",
  确认解除: "Confirm removal",
  确认删除: "Confirm deletion",
  "让 AI 提供删除建议与简短理由。是否启用由你决定，它不会操作文件。":
    "Let AI provide deletion advice and a short reason. Enabling it is optional, and it never operates on files.",
  日志与诊断: "Logs and diagnostics",
  "扫描 {1}": "Scan {1}",
  "扫描 {1} 盘": "Scan drive {1}",
  "扫描耗时取决于文件数量。": "Scan time depends on the number of files.",
  "扫描后可查看空间分布与清理建议。":
    "After scanning, you can view space distribution and cleanup suggestions.",
  扫描记录: "Scan records",
  "扫描记录保留数量（每个扫描位置）": "Scan record retention (per location)",
  扫描记录列表: "Scan record list",
  扫描记录与授权文本: "Scan records and consent text",
  扫描结果中的文件大小: "File sizes in scan results",
  扫描设置: "Scan settings",
  扫描所选磁盘: "Scan selected disk",
  扫描完成: "Scan complete",
  "扫描完成后，自动分析来源未明确的大文件":
    "Automatically analyze large files with unclear origins after scanning",
  扫描完成后才能加入待清理清单:
    "Finish the scan before adding items to the cleanup list",
  扫描文件夹: "Scan folder",
  扫描中: "Scanning",
  "删除会发生什么？": "What will deletion do?",
  删除扫描记录: "Delete scan record",
  "删除扫描记录：{1} · {2}": "Delete scan record: {1} · {2}",
  删除这条扫描记录: "Delete this scan record",
  "上下文：": "Context:",
  上一级: "Up one level",
  上一页: "Previous page",
  尚未执行清理操作: "No cleanup operations yet",
  设置: "Settings",
  "设置已保存；密钥只存入 Windows 凭据管理器。":
    "Settings saved. The key is stored only in Windows Credential Manager.",
  "设置已保存。服务地址已更改，自动发送与历史参考授权已重置，请重新检查并授权。":
    "Settings saved. Because the service address changed, automatic sending and history-reference consent were reset. Review and authorize them again.",
  社区规则: "Community rules",
  "社区来源、版本与转换报告":
    "Community source, version, and conversion report",
  失败: "Failed",
  识别规则: "Identification rules",
  实际占用: "Disk usage",
  "使用服务商提供的接口地址，或本机运行的 AI 服务地址。":
    "Use the API address supplied by the provider or an AI service running locally.",
  使用建议值: "Use suggested values",
  视频文件: "Video files",
  "试试其他文件名，或清除筛选。": "Try another file name or clear the filters.",
  收起: "Collapse",
  收起列表: "Collapse list",
  手动标注归属: "Set ownership manually",
  手动填写服务提供的模型名: "Enter the model name provided by the service",
  受保护: "Protected",
  授权读取所选文件并预览片段:
    "Allow reading selected files and preview snippets",
  输出兼容模式: "Output compatibility mode",
  输出长度参数: "Output length parameter",
  "输入 {1} / 输出 {2} tokens": "Input {1} / output {2} tokens",
  刷新: "Refresh",
  刷新分析记录: "Refresh analysis records",
  搜索当前目录: "Search current folder",
  搜索路径: "Search paths",
  搜索路径或文件名: "Search paths or file names",
  搜索清理建议: "Search cleanup suggestions",
  搜索文件名或路径: "Search file names or paths",
  搜索应用: "Search applications",
  搜索应用名称或所在路径: "Search application names or paths",
  搜索应用文件: "Search application files",
  "它可能是什么？": "What could it be?",
  提示: "Tip",
  天没有变化时才考虑推荐清理:
    " days without changes before recommending cleanup",
  添加到待清理清单: "Add to cleanup list",
  "填写 AI 服务商提供的兼容接口地址，通常以 /v1 结尾；不是聊天网页的网址。支持 HTTPS 云服务和本机运行的兼容服务，不会替你下载模型。":
    "Enter a compatible API address from your AI provider, usually ending in /v1; this is not the URL of a chat webpage. HTTPS cloud services and compatible local services are supported. DiskVista does not download models for you.",
  "填写服务商列出的模型标识，必须与其要求一致。不同于你给模型起的昵称。":
    "Enter the model identifier listed by the provider exactly as required. This is not a nickname you assign to the model.",
  "填写值超过参考最大输出，请确认服务实际限制；仍可保留手填值。":
    "The entered value exceeds the reference maximum output. Confirm the service limit; you can still keep the manual value.",
  条: " entries",
  跳过: "Skipped",
  同时发送的回收历史: "Recycling history sent with the request",
  "同一次扫描允许发送的 AI 请求总数，包括手动分析、自动分析及兼容性重试。达到上限后停止发送，不会悄悄追加请求。":
    "Total AI requests allowed for one scan, including manual analysis, automatic analysis, and compatibility retries. Sending stops at the limit; requests are never added silently.",
  "同一时间最多发送几个 AI 请求。数量较少时对网络和服务商的压力更小；首版最多同时发送 2 个。":
    "Maximum AI requests sent at once. Lower values reduce load on the network and provider; this version supports at most two concurrent requests.",
  图片: "Images",
  脱敏元数据: "Redacted metadata",
  外接存储: "External storage",
  "完成后即可查看清理建议。":
    "Cleanup suggestions will be available when scanning finishes.",
  "完成后可按“AI 已分析”筛选，每个文件查看删除建议与简短理由。":
    "When complete, filter by AI analyzed and view deletion advice and a short reason for each file.",
  未成功: "Unsuccessful",
  未归属部分: "Unassigned content",
  "未归属部分 · 已扣除单独列出的应用":
    "Unassigned content · separately listed applications excluded",
  未能扫描的位置与原因: "Locations not scanned and reasons",
  未能扫描的原因: "Reason not scanned",
  未扫描完整: "Incomplete scan",
  未识别用途: "Unidentified purpose",
  未收录: "Not listed",
  未完成: "Incomplete",
  未知: "Unknown",
  文档: "Documents",
  文件: "File",
  "文件基本信息（元数据）": "Basic file information (metadata)",
  文件夹: "Folder",
  文件列表: "File list",
  文件数量: "File count",
  文件位置: "File locations",
  文件详情: "File details",
  "文件只移入回收站。关闭程序后，不会继续在后台扫描。":
    "Files are moved only to the Recycle Bin. Scanning does not continue in the background after DiskVista closes.",
  文件总大小: "Total file size",
  "我允许自动模式发送上述基本信息（脱敏元数据，不含文件正文）":
    "Allow automatic mode to send the basic information above (redacted metadata, no file contents)",
  "无法读取具体原因，请稍后重试。":
    "The specific reason could not be read. Try again later.",
  无法读取清理建议: "Unable to load cleanup suggestions",
  "无法读取这个目录的空间分布，请重试。":
    "Unable to load space usage for this folder. Try again.",
  无法连接程序: "Unable to connect to the application",
  "系统保护默认开启。你可以在文件详情中保护某个位置、忽略项目或禁止发送给 AI，在这里取消这些设置。":
    "System protection is enabled by default. You can protect a location, ignore an item, or exclude it from AI in File Details, and remove those settings here.",
  "系统文件、驱动和已安装程序不能直接清理。":
    "System files, drivers, and installed programs cannot be cleaned directly.",
  下一页: "Next page",
  先选择一个扫描位置: "Select a scan location first",
  "相似之处：": "Similarities:",
  项: " items",
  "项 · 每页 100 项": " items · 100 per page",
  "项 · 占用约": " items · about",
  "项…": " items…",
  "项（约": " items (about",
  卸载应用: "Uninstall applications",
  修改时间: "Modified",
  需要核实: "Needs verification",
  需要你确认: "Needs your review",
  "选取依据：": "Selection basis:",
  "选择 {1}": "Select {1}",
  选择本页文件: "Select files on this page",
  "选择磁盘或文件夹，查看空间占用。":
    "Select a disk or folder to view space usage.",
  选择此项: "Select this item",
  "选择接口接收输出预算的参数名，数值由“单次输出上限”设置。不同服务支持的参数名不同；选择“不发送长度参数”时不会发送该数值。":
    "Choose the parameter name the API uses for the output budget. Its value comes from Output limit per request. Services support different parameter names; selecting Do not send a length parameter omits the value.",
  选择扫描记录: "Select scan record",
  选择文件夹: "Choose folder",
  选择要扫描的磁盘: "Select a disk to scan",
  选择这一类: "Select this category",
  压缩包: "Archives",
  "严格 JSON Schema": "Strict JSON Schema",
  "一次扫描保存下来的结果，方便之后查看或比较。它不会实时跟随磁盘变化；文件有变化时，请重新扫描。":
    "A saved result from one scan for later viewing or comparison. It does not track disk changes in real time; scan again when files change.",
  移出清单: "Remove from list",
  移除: "Remove",
  移除已保存密钥: "Remove saved key",
  移入回收站: "Move to Recycle Bin",
  "已安全保存，留空保持不变": "Saved securely; leave blank to keep unchanged",
  已标注的数据: "Labeled data",
  已过期: "Expired",
  "已回收 ·": "Recycled ·",
  已加入待清理清单: "Added to cleanup list",
  "已请求取消尚未开始的项目，正在等待操作结果。":
    "Cancellation was requested for items that have not started. Waiting for results.",
  已请求取消剩余项: "Cancellation requested for remaining items",
  已取消: "Cancelled",
  已识别应用: "Application identified",
  已识别用途: "Purpose identified",
  "已添加 {1} 项到待清理清单": "Added {1} items to the cleanup list",
  已跳过: "Skipped",
  已选: "Selected",
  已选清理项目: "Selected cleanup items",
  已移入回收站: "Moved to Recycle Bin",
  已用: "Used",
  已有扫描记录正在删除: "A scan record is already being deleted",
  已在待清理清单中: "Already in the cleanup list",
  "以下信息将发送到你设置的 AI 服务。请检查文件名是否包含隐私；不会发送全盘文件记录或完整应用列表。":
    "The following information will be sent to your configured AI service. Check file names for private information. A full-disk file record and the complete application list are never sent.",
  音频文件: "Audio files",
  应用标题栏: "Application title bar",
  应用或用途名称: "Application or purpose name",
  应用空间: "Application Space",
  应用如何分组: "How applications are grouped",
  应用数据: "Application data",
  应用文件: "Application files",
  应用文件夹: "Application folders",
  应用文件排序: "Application file sorting",
  应用与文件夹占用: "Application and folder usage",
  "影响：": "Impact:",
  "硬链接是多个文件路径指向同一份磁盘数据。占用统计会避免重复计算，但删除其中一个路径不一定能释放这份空间。":
    "Hard links are multiple file paths pointing to the same disk data. Usage statistics avoid double counting, but deleting one path may not free that space.",
  用户保护路径: "User-protected paths",
  用量未知: "Usage unknown",
  "用途：{1} 清理影响：{2}": "Purpose: {1} Cleanup impact: {2}",
  与删除历史相似: "Similar to recycling history",
  预览将发送的信息: "Preview information to be sent",
  元数据: "Metadata",
  "元数据就是文件的基本信息，例如名称、所在位置、大小和时间，不包含文件正文。文件名也可能包含隐私，发送前仍要检查。":
    "Metadata is basic file information such as name, location, size, and time; it does not include file contents. File names may still contain private information and should be reviewed before sending.",
  允许分析时参考并发送相关回收历史:
    "Allow analysis to reference and send related recycling history",
  "允许自动发送上述脱敏元数据（不含文件正文）":
    "Allow automatic sending of the redacted metadata above (no file contents)",
  "在设置中启用 AI": "Enable AI in Settings",
  "在设置中启用 AI，辅助解释用途不明的文件。":
    "Enable AI in Settings to help explain files with unclear purposes.",
  在资源管理器中查看: "View in File Explorer",
  "暂未识别此模型，可继续手动填写。":
    "This model is not recognized yet. You can continue with a manual value.",
  暂无: "None",
  增强扫描: "Enhanced scanning",
  展开: "Expand",
  "展开 {1} 项": "Expand {1} items",
  "展开应用可查看安装与数据位置，未归属内容单独列出。应用识别仅供参考，不代表可以删除。":
    "Expand an application to view installation and data locations. Unassigned content is listed separately. Application identification is for reference only and does not mean the files can be deleted.",
  占用: "Usage",
  占用空间: "Space used",
  "这不代表磁盘没有可清理的文件，可以查看其他类别后自行确认。":
    "This does not mean the disk has no cleanable files. Review other categories and decide for yourself.",
  这次扫描没有完成: "This scan did not finish",
  "这份扫描记录没有保留具体原因。可以重新扫描后查看。":
    "This scan record did not retain a specific reason. Scan again to inspect it.",
  这个目录没有已知的非零大小文件:
    "This folder has no known files with a non-zero size",
  这里暂时没有项目: "No items here yet",
  "这是单次请求的生成预算；服务支持的最大输出需单独确认，1M 上下文不等于输出上限。":
    "This is the generation budget for one request. Confirm the service's maximum output separately; a 1M context window is not the output limit.",
  "这是其余项目的合计，请在文件列表中选择具体项目。":
    "This is the total for the remaining items. Select specific items from the file list.",
  "这条分析未保存可读的依据详情。":
    "This analysis did not save readable evidence details.",
  "这些内容不能由本程序直接清理。系统空间和已安装应用可以通过 Windows 管理。":
    "DiskVista cannot clean this content directly. Manage system space and installed applications through Windows.",
  "这些位置的占用可能没有统计完整。不会为了读取它们而修改文件权限。":
    "Usage for these locations may be incomplete. DiskVista does not change file permissions to read them.",
  "这些文件在磁盘上实际占用的空间。压缩文件、共享同一份数据的文件等会让它与逻辑大小不同；无法确认时会标明估算或未知。":
    "The space these files actually occupy on disk. Compression and shared data can make it differ from logical size; uncertain values are marked as estimated or unknown.",
  正文采样: "Content sampling",
  "正在测试…": "Testing…",
  "正在读取…": "Loading…",
  "正在读取操作历史…": "Loading operation history…",
  正在读取设置和扫描记录: "Loading settings and scan records",
  正在读取文件信息: "Loading file information",
  "正在回收，请等待操作完成": "Recycling is in progress. Wait for it to finish",
  正在汇总: "Summarizing",
  "正在汇总目录空间…": "Summarizing folder space…",
  正在汇总占用和用途: "Summarizing usage and purpose",
  "正在分析本批 {1} 个文件…": "Analyzing this batch of {1} files…",
  "正在准备 {1} 个文件的批量分析…": "Preparing batch analysis for {1} files…",
  "正在按目录组织批量分析…": "Organizing batch analysis by folder…",
  "正在解除…": "Removing…",
  "正在进行安全检查，通过后将直接移入回收站…":
    "Running safety checks. Approved items will be moved directly to the Recycle Bin…",
  正在扫描: "Scanning",
  "正在扫描，完成后可查看各应用和文件夹的占用。":
    "Scanning. Application and folder usage will be available when complete.",
  正在扫描这个目录: "Scanning this folder",
  "正在删除…": "Deleting…",
  "正在生成清理建议。": "Generating cleanup suggestions.",
  "正在刷新…": "Refreshing…",
  正在添加: "Adding",
  "正在添加…": "Adding…",
  "正在统计占用…": "Calculating usage…",
  "正在选择…": "Selecting…",
  "正在移入回收站，请等待操作完成。":
    "Moving items to the Recycle Bin. Wait for the operation to finish.",
  "正在移入回收站…": "Moving to Recycle Bin…",
  "正在载入扫描结果…": "Loading scan results…",
  "正在整理清理建议…": "Preparing cleanup suggestions…",
  正在准备扫描: "Preparing scan",
  "只读取你本次选中的少量文本片段：最多 4 个 UTF-8 文本文件，每个最多 4 KiB。UTF-8 是常见的文字编码；不支持的文件、密码、钱包等敏感内容不会采样。读取后还需你确认才能发送。":
    "Read only small text snippets selected for this request: up to four UTF-8 text files, 4 KiB each. UTF-8 is a common text encoding. Unsupported files and sensitive content such as passwords and wallets are never sampled. Sending still requires your confirmation after previewing the content.",
  "只分析超过门槛的文件，不包含目录或已明确所属应用的文件。按目录分批分析，每个文件给出删除建议与简短理由。":
    "Analyze only files above the threshold, excluding folders and files already linked to applications. Files are analyzed in folder-based batches, with deletion advice and a short reason for each.",
  只扫描当前用户的临时文件夹: "Scan only the current user's temporary folder",
  置信度: "Confidence",
  中: "Medium",
  重试: "Retry",
  "重试也计入请求上限。没有服务商提供的用量或价格时，不估算费用。":
    "Retries count toward the request limit. Costs are not estimated without provider usage or pricing data.",
  重新扫描: "Scan again",
  重新统计: "Recalculate",
  重要内容会受到保护: "Important content is protected",
  重要内容默认受保护: "Important content is protected by default",
  准备扫描: "Ready to scan",
  "自动（推荐）": "Auto (recommended)",
  "自动分析待授权：允许发送基本信息并保存后，将在下次扫描完成时分析符合条件的项目。":
    "Automatic analysis needs consent. Allow sending basic information and save; eligible items will be analyzed after the next scan.",
  自动分析门槛: "Automatic analysis threshold",
  "自动分析门槛（MiB）": "Automatic analysis threshold (MiB)",
  总览: "Overview",
  最大并发: "Maximum concurrency",
  最大化窗口: "Maximize window",
  "最低按 100 MiB 生效，只分析严格大于门槛的文件。":
    "The minimum effective threshold is 100 MiB. Only files strictly larger than the threshold are analyzed.",
  "最多选择 4 个文本文件，每个读取不超过 4 KiB。先同意读取并查看片段，再决定是否发送。":
    "Select up to four text files and read no more than 4 KiB from each. First allow reading and review the snippets, then decide whether to send them.",
  最后访问: "Last accessed",
  最后访问时间: "Last access time",
  "最后访问时间由 Windows 记录，可能延迟更新，也可能没有记录。它不等于你最后一次打开应用的时间，不能仅凭很久没访问就决定删除。":
    "Last access time is recorded by Windows and may be delayed or unavailable. It is not the last time an application was opened and cannot by itself justify deletion.",
  "最近 10 次": "Latest 10",
  "最近 100 次": "Latest 100",
  "最近 30 次": "Latest 30",
  "最近 5 次": "Latest 5",
  "最近变化 {1}": "Latest change {1}",
  "最近变化：从旧到新": "Latest change: oldest first",
  "最近变化：从新到旧": "Latest change: newest first",
  最小化窗口: "Minimize window",
  "AI 标识暂不可用，仍可打开详情查看":
    "AI status is temporarily unavailable. You can still open details",
  "AI 待配置": "AI setup required",
  "AI 分析 · 本次发送预览": "AI analysis · sending preview",
  "AI 分析：已处理 {1}/{2} 项，请求 {3}/{4}":
    "AI analysis: {1}/{2} items processed, {3}/{4} requests",
  "AI 分析当前扫描": "Analyze current scan with AI",
  "AI 分析可能出错。服务商可能按其隐私政策保存收到的信息，请确认后再发送。":
    "AI analysis may be incorrect. The provider may retain received information under its privacy policy. Review before sending.",
  "AI 分析筛选": "AI analysis filter",
  "AI 分析中": "AI analysis in progress",
  "AI 辅助解释": "AI-assisted explanation",
  "AI 未分析": "Not analyzed by AI",
  "AI 未启用": "AI disabled",
  "AI 已分析": "Analyzed by AI",
  "AI 已分析仅包含当前有效结果；失败和过期结果归为未分析。":
    "Analyzed by AI includes only currently valid results. Failed and expired results count as not analyzed.",
  "AI 已过期": "AI analysis expired",
  "AI 只提供建议，不会操作文件。发送前可查看信息；读取文件内容还需要你另外同意。":
    "AI only offers advice and never operates on files. You can preview information before sending, and reading file contents requires separate consent.",
  "AI 准备分析…": "Preparing AI analysis…",
  "API 地址": "API address",
  "API 密钥": "API key",
  "API 密钥用于验证你使用 AI 服务的权限，可从服务商处获取。密钥保存在 Windows 凭据存储中；本机无需密钥的服务可以留空。":
    "An API key authenticates access to an AI service and is available from the provider. The key is stored in Windows Credential Manager; leave it blank for a local service that does not require a key.",
  "JSON 模式": "JSON mode",
  "JSON 文本": "JSON text",
  "Token 不等于字数，用量以服务商返回的数据为准，输出统计是否包含推理由服务决定。批量分析显示整批文件合计用量，不是每个文件分别消耗；缺少数据时不估算。":
    "Tokens are not words. Usage follows data returned by the provider, and whether output usage includes reasoning depends on the service. Batch analysis shows the total for the whole batch, not a separate cost per file. Missing usage is not estimated.",
  "Token 用量": "Token usage",
  "Windows 存储设置": "Windows Storage settings",
  "Windows 系统": "Windows system",
  "Windows 系统目录": "Windows system folder",
  完整扫描: "Full scan",
  "USN 验证复用": "USN validated reuse",
  "USN 增量扫描": "USN incremental scan",
  "变化目录无法可靠复用，回退完整扫描":
    "Changed folders could not be reused reliably; falling back to a full scan",
  "汇总目录、硬链接和不完整区域":
    "Summarizing folders, hard links, and incomplete areas",
  "扫描已取消，索引不完整，不能据此清理":
    "Scan cancelled. The index is incomplete and cannot be used for cleanup",
  "扫描根目录不作为整体清理目标，请逐层选择":
    "The scan root cannot be cleaned as a whole. Select items inside it",
  扫描根目录不能整体清理: "The scan root cannot be cleaned as a whole",
  扫描根目录不能解除保护: "Protection cannot be removed from the scan root",
  扫描根目录不能整体回收: "The scan root cannot be recycled as a whole",
  "目标不完整或包含受保护后代，不可整体回收":
    "The target is incomplete or contains protected descendants and cannot be recycled as a whole",
  "此目录包含当前受保护的路径，不可整体回收":
    "This folder contains a protected path and cannot be recycled as a whole",
  "尚未识别的目录，需要结合内容和来源判断":
    "Unidentified folder; review its contents and origin",
  "尚未识别的文件，不能仅凭大小或日期判断无用":
    "Unidentified file; size or date alone cannot determine that it is unnecessary",
  "删除可能影响应用功能或丢失个人数据；当前证据不足":
    "Deletion may affect application features or lose personal data; current evidence is insufficient",
  "只提供回收站恢复；不保证可重新生成":
    "Recovery is available only through the Recycle Bin; regeneration is not guaranteed",
  "请确认是否仍需要这些文件，可查看判断依据或请求 AI 分析":
    "Confirm whether these files are still needed. Review the evidence or request AI analysis",
  "可考虑回收；请确认不再需要并先关闭所属应用":
    "Consider recycling after confirming the item is no longer needed and closing the related application",
  "近期仍有活动，请先复核用途":
    "Recent activity detected. Review the purpose first",
  规则警告: "Rule warning",
  用户标注: "User label",
  本地判断: "Local assessment",
  扫描摘要: "Scan summary",
  扫描文件: "Scanned file",
  "扫描下载文件夹，查找安装包、视频等大文件":
    "Scan Downloads for installers, videos, and other large files",
  成功回收记录: "Successful recycling record",
  上次扫描记录: "Previous scan record",
  "Windows 卸载清单": "Windows uninstall registry",
  "Windows MSIX 包清单": "Windows MSIX package inventory",
  "开始菜单快捷方式（并非安装/创建者证明）":
    "Start menu shortcut (not proof of installation or creation)",
  "Epic 应用清单": "Epic application inventory",
  "Steam 应用清单": "Steam application inventory",
  已知应用数据位置: "Known application data location",
  "Windows 包家族标识": "Windows package family identifier",
  应用数据目录名称线索: "Application data folder naming clue",
  扫描中的可执行文件布局: "Executable layout found during scanning",
  程序版本资源: "Program version resources",
  程序声明的产品名: "Product name declared by the program",
  程序声明的公司: "Company declared by the program",
  程序声明的文件说明: "File description declared by the program",
  离线数字签名检查: "Offline digital signature check",
  "本机缓存信任链验证通过；不访问网络，不代表文件可以删除":
    "The locally cached trust chain passed verification without network access; this does not mean the file can be deleted",
  "Windows 标准目录位置": "Standard Windows folder location",
  "名称线索（非创建者证明）": "Naming clue (not proof of creation)",
  "目录名与应用名相似，可能共享或误匹配":
    "The folder name resembles the application name and may be shared or matched incorrectly",
  "程序安装总目录，包含多个独立应用及共享组件":
    "Program installation root containing multiple independent applications and shared components",
  "整体删除会破坏多个应用；请进入应用空间查看各应用占用，并通过系统应用管理处理":
    "Deleting the whole folder would break multiple applications. Review each application's usage in Application Space and manage it through Windows Apps settings",
  "依赖各应用安装程序修复或重新安装，不保证能完整恢复":
    "Recovery depends on each application's installer or reinstallation and may be incomplete",
  "Windows 操作系统目录，包含系统文件、驱动及系统组件":
    "Windows operating system folder containing system files, drivers, and components",
  "删除可能导致系统无法启动或功能损坏，只能通过系统入口管理":
    "Deletion may prevent Windows from starting or damage features. Manage this content only through Windows",
  可能需要系统修复或备份恢复:
    "System repair or restoration from backup may be required",
  "多应用数据总目录，包含配置、缓存及用户数据；应按应用和用途拆分查看":
    "Shared application data root containing settings, caches, and user data. Review it by application and purpose",
  "不能将整个目录视为缓存；删除可能丢失配置和个人数据":
    "The whole folder cannot be treated as cache; deletion may lose settings and personal data",
  "不同应用数据恢复方式不同，不能保证可重新生成":
    "Recovery differs by application and regeneration is not guaranteed",
  用户临时文件: "User temporary files",
  "Windows / 应用临时数据": "Windows / application temporary data",
  应用运行期间生成的临时数据: "Temporary data generated while applications run",
  "仍在使用的临时文件不会回收；部分应用可能重新生成数据":
    "Temporary files still in use are not recycled; some applications may regenerate data",
  "回收站恢复；可再生成的数据由应用重新生成":
    "Restore from the Recycle Bin; reproducible data is regenerated by the application",
  用户崩溃转储: "User crash dumps",
  "Windows 错误报告": "Windows Error Reporting",
  软件崩溃时保存的诊断材料: "Diagnostic data saved when software crashes",
  "失去对应崩溃的排错记录，不影响原应用文件":
    "Removes troubleshooting records for the crash without affecting the original application files",
  "可从回收站恢复；旧的崩溃现场不能重新生成":
    "Restore from the Recycle Bin; old crash state cannot be regenerated",
  "Chrome 缓存": "Chrome cache",
  "Edge 缓存": "Edge cache",
  "Firefox 缓存": "Firefox cache",
  "网页资源缓存，不含密码和浏览历史":
    "Web resource cache without passwords or browsing history",
  之后访问网页可能需要重新下载资源:
    "Web resources may need to be downloaded again",
  "应用联网后重新生成；也可从回收站恢复":
    "Regenerated when the application reconnects, or restored from the Recycle Bin",
  "Firefox 本地网页缓存": "Firefox local web cache",
  网页资源需要重新下载: "Web resources must be downloaded again",
  应用重新生成或从回收站恢复:
    "Regenerated by the application or restored from the Recycle Bin",
  "npm 下载缓存": "npm download cache",
  包管理器下载的缓存副本: "Cached copies downloaded by the package manager",
  "以后安装依赖可能需要联网重新下载；离线安装可能受影响":
    "Future dependency installation may require downloading again; offline installation may be affected",
  "联网后重新下载，或从回收站恢复":
    "Download again when online, or restore from the Recycle Bin",
  "pip 下载缓存": "pip download cache",
  "Python 软件包下载缓存": "Python package download cache",
  重新安装包可能需要联网和重新构建:
    "Reinstalling packages may require internet access and rebuilding",
  重新下载或从回收站恢复: "Download again or restore from the Recycle Bin",
  "VS Code 缓存": "VS Code cache",
  "编辑器界面资源缓存，不是工作区或配置":
    "Editor interface resource cache, not workspaces or settings",
  "下次启动可能变慢，缓存会重新生成":
    "The next launch may be slower while the cache regenerates",
  "Direct3D 着色器缓存": "Direct3D shader cache",
  图形应用编译后的着色器缓存:
    "Compiled shader cache from graphics applications",
  "图形应用可能重新编译着色器，短时间出现卡顿":
    "Graphics applications may recompile shaders and stutter briefly",
  运行图形应用后重新生成或从回收站恢复:
    "Regenerated after running graphics applications, or restored from the Recycle Bin",
  "社区规则识别的缓存、临时或日志目录；用途仍需人工复核":
    "Cache, temporary, or log folder identified by a community rule; its purpose still requires review",
  "可能丢失诊断记录、离线缓存或需要重新下载；不能保证可再生成":
    "May lose diagnostic records or offline caches, or require downloading again; regeneration is not guaranteed",
  "优先从 Windows 回收站恢复，不保证应用能重新生成":
    "Prefer restoration from the Windows Recycle Bin; the application may not be able to regenerate it",
  大文件: "Large files",
  "大于 100 MiB 的文件。请先确认里面的内容是否还需要。":
    "Files larger than 100 MiB. Confirm whether their contents are still needed.",
  "可能包含视频、安装包或个人资料；文件大不代表可以删除。":
    "May contain videos, installers, or personal data. A large file is not necessarily safe to delete.",
  "用户手动标注；用途与删除风险仍独立判断":
    "Labeled manually by the user; purpose and deletion risk are still assessed independently",
  "此目录中尚未归属的内容；已扣除其下独立识别的应用":
    "Unassigned content in this folder; separately identified applications below it are excluded",
  扫描根目录的其余内容: "Remaining content in the scan root",
  "扫描根目录中的未归属内容；不是一个已识别应用":
    "Unassigned content in the scan root; not an identified application",
  元数据预览不存在: "Metadata preview does not exist",
  预览已过期: "Preview expired",
  分析已取消: "Analysis cancelled",
  "AI 已关闭": "AI is disabled",
  "正在等待 AI 返回…": "Waiting for the AI response…",
  "已有 AI 分析任务运行，请等待或取消":
    "An AI analysis task is already running. Wait for it or cancel it",
  "连接成功，模型已回复。": "Connection successful. The model responded.",
  "连接成功，服务已接受测试请求。":
    "Connection successful. The service accepted the test request.",
  回收执行期间不能更改安全设置:
    "Safety settings cannot be changed while recycling is in progress",
  已有回收操作正在运行: "A recycling operation is already running",
  "预览不存在、已过期或已使用":
    "The preview does not exist, has expired, or was already used",
  "分类依据已变化，请重新加载":
    "Classification evidence changed. Reload the data",
  扫描记录不匹配: "Scan record mismatch",
  "扫描记录已变化，请刷新后重试":
    "The scan record changed. Refresh and try again",
  "此项已移入回收站，无需再次加入清单。重新扫描可更新文件状态。":
    "This item is already in the Recycle Bin and does not need to be added again. Scan again to refresh file status.",
  "此目录在扫描后已有内容被回收，请重新扫描后再选择整个目录。":
    "Content in this folder was recycled after scanning. Scan again before selecting the whole folder.",
  "正在回收，暂时不能开始新扫描":
    "A new scan cannot start while recycling is in progress",
  已有扫描正在运行: "A scan is already running",
  "未找到扫描 Worker，请使用完整安装包或运行构建脚本":
    "The scan worker was not found. Use the complete package or run the build script",
  "Worker 输出不可用": "Worker output is unavailable",
  "Worker 输入不可用": "Worker input is unavailable",
  "Worker 意外结束，结果不完整，请重新扫描":
    "The worker exited unexpectedly. Results are incomplete; scan again",
  简体中文: "Simplified Chinese",
  英文: "English",
  语言: "Language",
  界面语言: "Interface language",
  "选择应用界面使用的语言。":
    "Choose the language used by the application interface.",
};

function templatePattern(source: string): RegExp {
  const escaped = source.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`^${escaped.replace(/\\\{\d+\\\}/g, "(.+?)")}$`);
}

function fromTemplate(
  source: string,
  target: string,
  value: string,
): string | null {
  if (!source.includes("{1}")) return null;
  const indexes = [...source.matchAll(/\{(\d+)\}/g)].map((match) =>
    Number(match[1]),
  );
  const match = templatePattern(source).exec(value);
  if (!match) return null;
  return target.replace(/\{(\d+)\}/g, (_, index: string) => {
    const position = indexes.indexOf(Number(index));
    return position >= 0 ? match[position + 1] : "";
  });
}

const templateEnglish = Object.entries(exactEnglish)
  .filter(([source]) => source.includes("{1}"))
  .sort(([left], [right]) => right.length - left.length);

export function englishText(value: string): string {
  const match = /^(\s*)(.*?)(\s*)$/s.exec(value);
  const leading = match?.[1] ?? "";
  const core = match?.[2] ?? value;
  const trailing = match?.[3] ?? "";
  const exact = exactEnglish[core];
  if (exact !== undefined) return `${leading}${exact}${trailing}`;
  for (const [source, target] of templateEnglish) {
    const translated = fromTemplate(source, target, core);
    if (translated !== null) return `${leading}${translated}${trailing}`;
  }
  return value;
}
