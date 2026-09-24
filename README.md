# 微信流 WeChatBridge for Windows

把微信「合并转发」导出的 ZIP 聊天记录，一键转发给 AI Agent（Claude / Codex / 豆包 / 通义千问 / WorkBuddy / WeSight / 自定义应用）、Obsidian 或剪贴板。

本项目是对 [freestylefly/WeChatBridge](https://github.com/freestylefly/WeChatBridge)（macOS 版，MIT）的 Windows 重写，行为规格见 [SPEC.md](./SPEC.md)。

## 安装

下载便携版 `WeChatBridge-portable.exe`，双击即可运行，无需安装。也提供 NSIS 安装包 `WeChatBridge_0.1.0_x64-setup.exe`。

## 快速开始（三步）

1. **获取 ZIP**：在微信里多选聊天记录 → 合并转发 → 发给好友「元宝AI」，它会自动生成 ZIP 压缩包；把 ZIP 保存到本地。
2. **拖入悬浮球**：启动应用后，屏幕上会出现一个紫色「微」字悬浮球（始终置顶）。把 ZIP 拖到悬浮球上，自动导入。
3. **一键转发**：在主窗口预览聊天记录，选择目标 AI 应用，点击「转发」即可。

> 也可以把 ZIP 拖到主窗口的拖放区，或点击选择文件。悬浮球误关后可在系统托盘菜单「显示悬浮球」找回。

## 功能

- **拖拽悬浮球**：始终置顶的小球，接收从微信/资源管理器拖入的 ZIP，导入后自动弹出主窗口。
- **多目标转发**：Claude / Codex / 豆包 / 千问 / WorkBuddy / WeSight / Obsidian / 剪贴板 / 自定义应用。
- **场景库**：5 个内置场景（客户复盘、项目周会、日常摘要等），转发时附带提示词。
- **Obsidian 归档**：导出为带 frontmatter 的 Markdown 笔记，附件存入 `附件/` 目录。
- **终端友好**：终端类目标自动使用「提示词 + 引用路径」单行文本，避免换行误执行。
- **系统集成**：托盘常驻、开机自启、单实例运行、关闭最小化到托盘。

## 隐私

聊天内容只来自用户主动导出的文件；不读取 / 解密 / 注入微信进程或数据库；全部数据保存在本机（`%APPDATA%\com.wechatbridge.windows\`），无网络遥测。

## 开发

技术栈：Tauri 2 + Rust + React 19 + TypeScript + Vite。

```powershell
npm install
npm run dev          # 前端开发
npm run tauri dev    # 应用开发调试
cargo test --lib     # Rust 单元测试（27 个）
npm run tauri build  # 产出便携版 exe（target/release/wechatbridge-windows.exe）
```

## 致谢与参考

> 本项目是对原 macOS 项目 **[freestylefly/WeChatBridge](https://github.com/freestylefly/WeChatBridge)** 的 Windows 平台移植与适配。
>
> - 行为规格、场景库、Obsidian 归档格式、粘贴计划（提示词优先 + 文件送达）等核心设计均移植自原项目。
> - 原项目以 **MIT License** 开源，本仓库同样以 MIT 发布。
> - 感谢原作者 freestylefly 的优秀工作。
