# 微信流 WeChatBridge for Windows

把微信「合并转发」导出的 ZIP 聊天记录，一键转发给 AI Agent（Claude / Codex / 豆包 / 通义千问 / WorkBuddy / WeSight / 自定义应用）、Obsidian 或剪贴板。

本项目是对 [freestylefly/WeChatBridge](https://github.com/freestylefly/WeChatBridge)（macOS 版，MIT）的 Windows 重写，行为规格见 [SPEC.md](./SPEC.md)。

## 安装

到 [Releases](https://github.com/kongkong08/WeChatBridge-Windows/releases) 下载便携版 `WeChatBridge-portable.exe`，双击即可运行，无需安装（仅依赖 Windows 10/11 自带的 WebView2）。

## 快速开始（三步）

1. **获取 ZIP**：在微信里多选聊天记录 → 合并转发 → 发给好友「元宝AI」，它会自动生成 ZIP 压缩包；把 ZIP 保存到本地。
2. **拖入悬浮球**：启动应用后，屏幕上会出现一个绿色「微」字悬浮球（始终置顶）。把 ZIP 拖到悬浮球上，自动导入。
3. **一键转发**：右键悬浮球点目标图标直接转发；或在主窗口预览聊天记录后转发。

> 进阶：设置页开启「拖入后自动转发到」后，拖入 ZIP 即完成「导入 + 转发」全流程，连右键都不用点。

## 悬浮球玩法

| 操作 | 行为 |
|---|---|
| 拖入 ZIP | 脉冲光环提示 → 自动导入（可配自动转发） |
| 拖拽 | 移动位置，松手自动贴边吸附 |
| 滚轮 | 无级缩放球体（48-120px），自动保存 |
| 右键 | 目标图标网格快捷转发（★ 标记上次目标）+ 复制 / 半隐藏 / 主窗口 / 隐藏 |
| 双击 / 单击 | 打开主窗口 |
| 半隐藏 | 球一半藏到屏幕边缘外，鼠标悬停自动弹出，移开自动收回 |
| 待处理提醒 | 有未转发批次时红色角标计数 + 呼吸光环 |
| 空闲 3 秒 | 自动半透明，鼠标靠近恢复 |
| 成功 / 失败 | 绿色波纹扩散 / 红色抖动反馈 |

## 全局快捷键

| 快捷键 | 功能 |
|---|---|
| `Ctrl + Shift + W` | 显示 / 隐藏悬浮球 |
| `Ctrl + Shift + M` | 打开主窗口 |

## 功能

- **多目标转发**：Claude / Codex / 豆包 / 千问 / WorkBuddy / WeSight / Obsidian / 剪贴板 / 自定义应用。
- **拖入自动转发**：零点击完成「导入 → 转发」全流程。
- **批量操作**：批次多选，批量转发 / 批量删除。
- **场景库**：5 个内置场景（客户复盘、项目周会、日常摘要等），转发时附带提示词。
- **Obsidian 归档**：导出为带 frontmatter 的 Markdown 笔记，附件存入 `附件/` 目录。
- **终端友好**：终端类目标自动使用「提示词 + 引用路径」单行文本，避免换行误执行。
- **系统集成**：托盘常驻（显示/隐藏悬浮球）、开机自启、单实例运行、关闭最小化到托盘、品牌图标。

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

品牌图标可通过 `powershell scripts/make-icons.ps1` 重新生成。

## 致谢与参考

> 本项目基于开源项目 **[Dukou（渡口）](https://github.com/qzz0518/Dukou)** 二次开发，感谢原作者 **qzz0518** 的开创性工作——开源太伟大了。
>
> 同时感谢 macOS 项目 **[freestylefly/WeChatBridge](https://github.com/freestylefly/WeChatBridge)**（同为基于 Dukou 的二次开发）：
>
> - 行为规格、场景库、Obsidian 归档格式、粘贴计划（提示词优先 + 文件送达）等核心设计移植自上述项目。
> - Dukou 与 freestylefly/WeChatBridge 均以 **MIT License** 开源，本仓库同样以 MIT 发布。
> - 站在开源的肩膀上，才能把微信聊天记录送到更远的地方。
