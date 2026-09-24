//! 批次（Batch）数据模型与状态机。
//!
//! 移植自 macOS 版 `BatchManifest.swift` / `BatchState.swift` / `ShareAction.swift`。
//! 状态机在原文四态（delivered/copied/failed/expired）上增加 `received` 初始态：
//! Windows 版由用户拖放触发、可能稍后才转发，需要显式的"已接收未处理"阶段。

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// 转发入口。与 macOS 版 ShareAction 对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShareAction {
    Codex,
    Claude,
    Doubao,
    Qwen,
    WorkBuddy,
    WeSight,
    Obsidian,
    Clipboard,
    Custom,
}

/// 批次结果状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BatchState {
    /// 已接收归档，尚未转发（Windows 版新增）。
    Received,
    /// 已送达目标应用（激活 + 粘贴完成）。
    Delivered,
    /// 已复制到剪贴板（兜底路径）。
    Copied,
    /// 转发失败。
    Failed,
    /// 归档过期（保留原始语义，供后续新鲜度策略使用）。
    Expired,
}

/// 批次清单：一次拖放产生的归档工作单元。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchManifest {
    pub id: String,
    pub created_at: NaiveDateTime,
    /// 来源入口（拖放创建时为 None，转发后记录实际使用的入口）。
    #[serde(default)]
    pub action: Option<ShareAction>,
    /// 原始 ZIP 文件名（展示用）。
    pub display_name: String,
    /// 归档目录相对存储根的路径。
    pub relative_path: String,
    #[serde(default = "BatchState::received")]
    pub state: BatchState,
    /// 识别到的聊天名（来自文件名或内容推断）。
    #[serde(default)]
    pub chat_name: Option<String>,
    /// 转发时使用的场景名（如有）。
    #[serde(default)]
    pub scene_name: Option<String>,
    /// 失败原因（state = failed 时）。
    #[serde(default)]
    pub error: Option<String>,
}

impl BatchState {
    fn received() -> Self {
        BatchState::Received
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_round_trip() {
        let manifest = BatchManifest {
            id: "20260923-143000".to_string(),
            created_at: NaiveDateTime::parse_from_str("2026-09-23 14:30", "%Y-%m-%d %H:%M").unwrap(),
            action: None,
            display_name: "聊天记录 2026.zip".to_string(),
            relative_path: "batches/20260923-143000".to_string(),
            state: BatchState::Received,
            chat_name: Some("产品讨论组".to_string()),
            scene_name: None,
            error: None,
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let back: BatchManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, manifest);
        assert!(json.contains("\"received\""));
    }
}
