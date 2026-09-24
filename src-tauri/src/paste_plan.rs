//! 粘贴计划：决定一次转发向目标应用粘贴什么、按什么顺序。
//!
//! 行为规格移植自 macOS 版 `PastePlan.swift`：
//! - 提示词永远在最前：附件先到会让 Agent 先开始猜任务
//! - 普通目标：提示词与文件分两次粘贴（一次 ⌘V 同时给文本和文件时，聊天类应用会选文件）
//! - 终端类目标：提示词压平成单行 + 引用路径拼成一行（换行在终端里等于回车执行）
//!
//! Windows 适配：终端路径引用从 POSIX 单引号改为双引号（cmd/PowerShell 通用；
//! Windows 路径本就不允许 `"` 字符，无需内层转义）。

use std::path::{Path, PathBuf};

/// 一次 Ctrl+V 承载的内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PastePayload {
    Files(Vec<PathBuf>),
    Text(String),
}

/// 生成粘贴计划。
pub fn make(urls: &[PathBuf], path_only: bool, prompt: Option<&str>) -> Vec<PastePayload> {
    let prompt = prompt.map(str::trim).filter(|p| !p.is_empty());

    if !path_only {
        return match prompt {
            None => vec![PastePayload::Files(urls.to_vec())],
            Some(p) => vec![PastePayload::Text(p.to_string()), PastePayload::Files(urls.to_vec())],
        };
    }

    let paths = shell_line(urls);
    match prompt {
        None => vec![PastePayload::Text(paths)],
        Some(p) => {
            let flat = p
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            vec![PastePayload::Text(format!("{flat} {paths}"))]
        }
    }
}

/// 自动粘贴被拒绝时，手动 Ctrl+V 应得到的内容：优先文件，否则第一项。
pub fn manual_payload(plan: &[PastePayload]) -> Option<&PastePayload> {
    plan.iter()
        .find(|p| matches!(p, PastePayload::Files(_)))
        .or_else(|| plan.first())
}

/// 与 macOS 版 `shellLine` 对应的 Windows 版本：双引号包裹 + 尾部空格。
pub fn shell_line(urls: &[PathBuf]) -> String {
    urls.iter()
        .map(|u| format!("{} ", shell_quoted(u)))
        .collect()
}

/// Windows 终端通用引用：双引号。路径不含 `"`（Windows 文件名禁止该字符），无需内层转义。
pub fn shell_quoted(path: &Path) -> String {
    format!("\"{}\"", path.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn urls() -> Vec<PathBuf> {
        vec![
            PathBuf::from(r"D:\archives\聊天记录 2026.zip"),
            PathBuf::from(r"D:\archives\导出附件"),
        ]
    }

    #[test]
    fn plain_target_files_only() {
        let plan = make(&urls(), false, None);
        assert_eq!(plan, vec![PastePayload::Files(urls())]);
    }

    #[test]
    fn plain_target_prompt_first_then_files() {
        let plan = make(&urls(), false, Some("  总结这个群聊  "));
        assert_eq!(
            plan,
            vec![
                PastePayload::Text("总结这个群聊".to_string()),
                PastePayload::Files(urls())
            ]
        );
    }

    #[test]
    fn terminal_paths_only() {
        let plan = make(&urls(), true, None);
        assert_eq!(
            plan,
            vec![PastePayload::Text(
                "\"D:\\archives\\聊天记录 2026.zip\" \"D:\\archives\\导出附件\" ".to_string()
            )]
        );
    }

    #[test]
    fn terminal_prompt_flattened_to_one_line() {
        let plan = make(&urls(), true, Some("第一行\n\n  第二行  \n第三行"));
        assert_eq!(
            plan,
            vec![PastePayload::Text(
                "第一行 第二行 第三行 \"D:\\archives\\聊天记录 2026.zip\" \"D:\\archives\\导出附件\" "
                    .to_string()
            )]
        );
    }

    #[test]
    fn manual_payload_prefers_files() {
        let plan = make(&urls(), false, Some("提示"));
        assert_eq!(manual_payload(&plan), Some(&PastePayload::Files(urls())));
        let terminal = make(&urls(), true, Some("提示"));
        assert_eq!(manual_payload(&terminal), terminal.first());
    }
}
