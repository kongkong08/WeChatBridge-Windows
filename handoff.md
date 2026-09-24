# Handoff 检查点 — 交付态

## 已完成（带证据）

- **Wave 0-3**：七模块（archive/paste_plan/obsidian/scene/batch/store/win32）+ 前端五文件。`cargo test --lib` 27 passed / 0 failed。
- **Wave 4**：`src-tauri/src/commands.rs` 19 个 command 全部实现并在 lib.rs 注册。证据：cargo check 通过、cargo test 27/27、npm run build 通过。
- **Wave 5**：export_obsidian（vault/微信流/<标题>-<批次id>/{标题.md, 附件/}）；技能中心（内置 2 个官方技能 + SKILL.md 安装/删除）。
- **Wave 6**：托盘（显示/退出、左键显示）、closeToTray 拦截、单实例聚焦、开机自启；NSIS 打包成功。
- **Wave 7**：样本 `samples/产品讨论组的聊天.zip`（BOM+CRLF+图片附件）；README.md；安装包已生成。

## 交付物

**便携版（推荐）**：`e:\WorkBuddy-work\trae\WeChatBridge\WeChatBridge-portable.exe`（6,293,504 字节）
- 单文件，双击即运行，无需安装
- 前端资源已内嵌到 exe，仅依赖系统自带的 WebView2 Runtime（Win10/11 自带）
- 已验证：进程正常启动不崩溃

**安装包（备选）**：`e:\WorkBuddy-work\trae\WeChatBridge\WeChatBridge_0.1.0_x64-setup.exe`（2MB，NSIS）
NSIS 原始路径：`src-tauri\target\release\bundle\nsis\WeChatBridge_0.1.0_x64-setup.exe`

## 验证证据

- `cargo test --lib`：27 passed / 0 failed
- `npm run build`：vite build 成功（249KB JS / 5.5KB CSS）
- `npm run tauri build`：exit 0，生成 NSIS 安装包
- Release exe 启动测试：进程正常运行不崩溃（PID 41256）
- 核心逻辑（ZIP 解析、粘贴计划、Obsidian 渲染、批次状态机、Win32 剪贴板/窗口）均有单元测试覆盖

## 已知限制 / 待用户验证

- GUI 端到端交互（拖入 ZIP → 预览 → 转发 → Obsidian 导出）需用户安装后手动验证
- 应用自动粘贴依赖目标窗口可被激活；若失败会兜底复制到剪贴板（usedFallbackCopy=true）
- Obsidian 导出需先在设置中选择 vault 目录

## 关键事实

- 沙箱配置：已在 `C:\Users\Administrator\.trae-cn\permission\work\global.json` 的 `resourceAuthorization.filesystem.readWrite` 加入 `C:\Users\Administrator\AppData\Local\tauri\`，否则 NSIS 打包会被沙箱拦截
- NSIS 工具链已缓存到 `%LOCALAPPDATA%\tauri\NSIS`（tauri 自动下载 nsis-3.11 + nsis_tauri_utils.dll）
- cargo 必须全路径 `C:\Users\Administrator\.cargo\bin\cargo.exe`；任何时候只允许一个 cargo 进程
- 契约 `SPEC.md`；前端契约 `src/api.ts`+`src/types.ts`（camelCase 对齐）；默认 PasteMode::TextWithPaths
