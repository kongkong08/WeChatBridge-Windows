# WeChatBridge Windows — 行为规格（重写契约）

> 从 macOS 版（`upstream/`，commit: main 分支）提取的重写契约。Windows 版实现必须与此一致，差异处在「Windows 适配」节注明。

## 1. 输入：微信「合并转发」ZIP

- 来源：macOS 微信 4.1.13+ 多选聊天记录 → 合并转发 → 导出 ZIP（含 `聊天记录.txt` + 图片/视频附件）。
- **Windows 版输入方式：拖放 ZIP 到主窗口 / 文件选择对话框**（Windows 微信无 Share Extension 机制）。
- ZIP 解析（`src-tauri/src/archive.rs`）：
  - 只读取 `.txt` 条目，优先精确名 `聊天记录.txt`；
  - 防 ZIP slip（拒绝 `..`、绝对路径、控制字符、冒号）；
  - 支持 store / deflate；UTF-8（容错 lossy）解码。

## 2. 聊天 TXT 格式

```
·发送者昵称
2024年1月5日 14:30
消息内容（可多行，直到下一条 `·` 开头）
```

- 正则：`(?m)^·([^\n]+)\n(\d{4}年\d{1,2}月\d{1,2}日 \d{2}:\d{2})\n`
- 去 BOM（`\u{feff}`）、归一 `\r\n → \n`；首条匹配必须位于位置 0，否则 `InvalidTranscript`。
- 时间格式：`yyyy年M月d日 HH:mm`（月日不补零）。

## 3. 批次（Batch）

- 字段：id / created_at / action?（ShareAction）/ display_name / relative_path / state / chat_name? / scene_name? / error?
- 状态机：`received → delivered / copied / failed`（`expired` 保留原语义待用）。`received` 为 Windows 版新增初始态。
- 持久化：`%APPDATA%/com.wechatbridge.windows/` 下 JSON（Wave 2 实现）。

## 4. 粘贴计划（PastePlan）

- **提示词永远在最前**；普通目标：`[Text(prompt), Files(urls)]` 两次粘贴；无提示词仅 `Files`。
- 终端类（pathOnly）：单行 `压平提示词 引用路径…`（提示词换行压平防终端误执行）。
- `manual_payload`：自动粘贴被拒时手动 Ctrl+V 的内容 —— 优先 Files，否则第一项。
- **Windows 适配**：路径引用用**双引号**（`"D:\a b\c.zip" `，尾部空格）；macOS 的 POSIX 单引号不适用于 cmd/PowerShell。Windows 路径禁止 `"` 字符，无需内层转义。
- Windows 终端白名单（pathOnly 默认开）：Windows Terminal / cmd / PowerShell / Git Bash。

## 5. Windows 系统集成（替代 macOS 机制）

| macOS | Windows |
|---|---|
| bundle id 定位 App | 进程名 / 窗口标题匹配 + 用户配置 exe 路径 |
| NSRunningApplication 激活 | EnumWindows → ShowWindow(SW_RESTORE) → SetForegroundWindow（焦点保护用 AttachThreadInput） |
| NSPasteboard 文件 | 剪贴板 CF_HDROP + SendInput Ctrl+V；**兜底：路径文本粘贴**（AI 应用对文件剪贴板支持不一，设置可选模式） |
| 辅助功能权限 | 无需权限（SendInput 为应用级 API） |

## 6. Obsidian 归档

- 标题：聊天名优先（补「的聊天」后缀）→ 参与者规则（1人 `X的聊天` / 2人 `X与Y的聊天` / 多人 `X等N人的聊天`）→ 归档文件名去扩展名。
- frontmatter：`title / source: WeChat / chat? / scene? / exported: yyyy-MM-dd HH:mm / messages: N / archive: "附件/<归档名>"`；字符串双引号 YAML 转义（`\` `"` `\n`）。
- 正文：`# 标题` + `> 来源：微信 · 原始归档：[[附件/...]]` + `## 聊天记录` + 每条 `**发送者** · 时间` + 空行 + 正文 + 附件链接。
- 附件链接：图片/视频/音频/PDF 用 `![[...]]`，其余 `[[...]]`；转义 `\ [ ] # ^ |`；附件保存到笔记旁 `附件/` 子目录；引用检测按名字长度降序消耗。

## 7. 场景（Scene）

- ScenePackage（可分发公开形态，schema v2）：id/name/version/author/applicability/keywords[]/instruction/outputSpec/requiredSkillIds[]/compatibleAgents[]/isOfficial。
- SceneSettings：scenes[] + defaultSceneId? + attachToForwards（默认 false）。
- SceneVersion：点分数字，忽略尾部零比较（`1.2 == 1.2.0 < 1.10`）。
- AgentID：codex / claude / doubao / qwen / workBuddy / weSight。

## 8. 内置入口（Windows 目标映射）

codex（Codex CLI/桌面）/ claude（Claude Desktop）/ doubao（豆包 PC）/ qwen / workBuddy / weSight / obsidian / clipboard / custom。
- 检测：运行中进程枚举 + 常见安装路径 + 用户自定义 exe。
- 未安装目标标注不可用；目标不可达时文件保留并兜底剪贴板。

## 9. 隐私红线（继承原项目）

- 聊天内容只来自用户主动提供的导出文件；不读取/解密/注入微信进程或数据库；
- 全部数据保存在本机；无网络遥测。

## 10. 验收口径（终极功能）

拖入微信导出 ZIP → 预览聊天记录 → 一键激活目标应用并完成粘贴（或归档 Obsidian），全程本地；`cargo test` 全绿；NSIS 安装包产出。
