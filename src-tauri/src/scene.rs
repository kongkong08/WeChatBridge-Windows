//! 场景（Scene）数据模型。
//!
//! 移植自 macOS 版 `Scene.swift`：
//! - ScenePackage 是可分发的公开形态（不含本地开关与群绑定）
//! - SceneVersion 为点分数字版本，比较时忽略尾部零
//! - SceneSettings 含场景库 + 默认场景 + 转发时是否附带场景的总开关

use serde::{Deserialize, Serialize};

/// 兼容的 Agent 标识。与 macOS 版 AgentID 对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentId {
    Codex,
    Claude,
    Doubao,
    Qwen,
    WorkBuddy,
    WeSight,
}

impl AgentId {
    pub const ALL: [AgentId; 6] = [
        AgentId::Codex,
        AgentId::Claude,
        AgentId::Doubao,
        AgentId::Qwen,
        AgentId::WorkBuddy,
        AgentId::WeSight,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            AgentId::Codex => "codex",
            AgentId::Claude => "claude",
            AgentId::Doubao => "doubao",
            AgentId::Qwen => "qwen",
            AgentId::WorkBuddy => "workBuddy",
            AgentId::WeSight => "weSight",
        }
    }
}

/// 本地场景实体。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeChatScene {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    pub instruction: String,
    #[serde(default)]
    pub output_spec: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_version")]
    pub package_version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub applicability: String,
    #[serde(default)]
    pub required_skill_ids: Vec<String>,
    #[serde(default = "all_agents")]
    pub compatible_agents: Vec<AgentId>,
    #[serde(default)]
    pub is_official: bool,
}

fn default_version() -> String {
    "1.0".to_string()
}

fn all_agents() -> Vec<AgentId> {
    AgentId::ALL.to_vec()
}

/// 可分发的场景包（schema v2）。导入新版本不覆盖本地开关。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenePackage {
    #[serde(default = "schema_v1")]
    pub schema_version: i32,
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub applicability: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub instruction: String,
    #[serde(default)]
    pub output_spec: String,
    #[serde(default)]
    pub required_skill_ids: Vec<String>,
    #[serde(default = "all_agents")]
    pub compatible_agents: Vec<AgentId>,
    #[serde(default)]
    pub is_official: bool,
}

fn schema_v1() -> i32 {
    1
}

impl ScenePackage {
    pub const CURRENT_SCHEMA_VERSION: i32 = 2;
}

/// 场景库与转发附带开关。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneSettings {
    pub scenes: Vec<WeChatScene>,
    #[serde(default)]
    pub default_scene_id: Option<String>,
    #[serde(default)]
    pub attach_to_forwards: bool,
}

/// 点分数字版本：`1.2` == `1.2.0` < `1.10`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneVersion {
    components: Vec<u64>,
}

impl SceneVersion {
    pub fn parse(raw: &str) -> Option<Self> {
        if raw.is_empty() {
            return None;
        }
        let mut values = Vec::new();
        for part in raw.split('.') {
            if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            values.push(part.parse::<u64>().ok()?);
        }
        while values.len() > 1 && values.last() == Some(&0) {
            values.pop();
        }
        Some(SceneVersion { components: values })
    }
}

impl PartialOrd for SceneVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SceneVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let count = self.components.len().max(other.components.len());
        for i in 0..count {
            let l = self.components.get(i).copied().unwrap_or(0);
            let r = other.components.get(i).copied().unwrap_or(0);
            match l.cmp(&r) {
                std::cmp::Ordering::Equal => {}
                ord => return ord,
            }
        }
        std::cmp::Ordering::Equal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison() {
        assert_eq!(SceneVersion::parse("1.2"), SceneVersion::parse("1.2.0"));
        assert!(SceneVersion::parse("1.2") < SceneVersion::parse("1.10"));
        assert!(SceneVersion::parse("2.0") > SceneVersion::parse("1.9.9"));
        assert!(SceneVersion::parse("1..2").is_none());
        assert!(SceneVersion::parse("1.2a").is_none());
        assert!(SceneVersion::parse("").is_none());
    }

    #[test]
    fn package_defaults_match_swift_decoder() {
        let json = r#"{"id":"s1","name":"总结","version":"1.0","instruction":"总结群聊"}"#;
        let pkg: ScenePackage = serde_json::from_str(json).expect("must decode");
        assert_eq!(pkg.schema_version, 1);
        assert!(pkg.author.is_empty());
        assert_eq!(pkg.compatible_agents.len(), 6);
        assert!(!pkg.is_official);
    }

    #[test]
    fn agent_id_serializes_like_swift() {
        assert_eq!(serde_json::to_string(&AgentId::WorkBuddy).unwrap(), "\"workBuddy\"");
        let back: AgentId = serde_json::from_str("\"workBuddy\"").unwrap();
        assert_eq!(back, AgentId::WorkBuddy);
    }
}
