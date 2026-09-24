//! 本地数据层：批次 / 场景 / 转发目标 / 应用设置的 JSON 持久化。
//!
//! 存储根：`%APPDATA%/com.wechatbridge.windows/`（测试中可注入任意目录）。
//!
//! 目录布局：
//! ```text
//! <root>/
//!   settings.json
//!   scenes.json
//!   targets.json
//!   batches/<batch_id>/
//!     manifest.json
//!     archive.zip        # 原始 ZIP 副本
//! ```
//!
//! 对应 macOS 版的 UserDefaults + Application Support 持久化；
//! Windows 版无 App Group 容器，统一放到 %APPDATA% 下。

use std::fs;
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::batch::{BatchManifest, BatchState};
use crate::scene::{AgentId, SceneSettings, WeChatScene};

/// 转发目标应用定义（Windows 版：按进程名 / 窗口标题 / 用户自定义 exe 匹配）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetApp {
    /// 稳定标识，如 "codex" / "claude" / "custom-<uuid>"。
    pub id: String,
    pub display_name: String,
    /// 运行中进程名（含 .exe），用于检测与激活。
    #[serde(default)]
    pub process_names: Vec<String>,
    /// 窗口标题包含的关键字（激活时辅助匹配）。
    #[serde(default)]
    pub window_title_keywords: Vec<String>,
    /// 用户自定义 exe 绝对路径（未安装到常见位置时兜底 / 用于启动）。
    #[serde(default)]
    pub exe_path: Option<String>,
    /// 终端类目标：只粘贴「提示词 + 引用路径」文本（SPEC §4）。
    #[serde(default)]
    pub path_only: bool,
    /// 是否内置入口（内置不可删除，可禁用）。
    #[serde(default)]
    pub builtin: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// 文件送达方式（SPEC §5 兜底策略）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PasteMode {
    /// 文件剪贴板（CF_HDROP）+ Ctrl+V：AI 对话窗口收到真正的文件（默认）。
    FileClipboard,
    /// 兜底：引用路径文本（不支持文件剪贴板的目标使用）。
    TextWithPaths,
}

impl Default for PasteMode {
    fn default() -> Self {
        PasteMode::FileClipboard
    }
}

/// 应用设置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// 界面语言："zh" / "en"。
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub paste_mode: PasteMode,
    /// Obsidian 仓库根目录（未设置时 Obsidian 导出不可用）。
    #[serde(default)]
    pub obsidian_vault: Option<String>,
    /// 开机自启（Wave 6 接入 autostart 插件）。
    #[serde(default)]
    pub autostart: bool,
    /// 关闭主窗口时最小化到托盘而不是退出。
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
    /// 悬浮球尺寸（像素，直径）。
    #[serde(default = "default_ball_size")]
    pub ball_size: u32,
    /// 悬浮球主题色（CSS 颜色，如 "#6c5ce7"）。
    #[serde(default = "default_ball_color")]
    pub ball_color: String,
}

fn default_language() -> String {
    "zh".to_string()
}

fn default_ball_size() -> u32 {
    72
}

fn default_ball_color() -> String {
    "#07c160".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            language: default_language(),
            paste_mode: PasteMode::default(),
            obsidian_vault: None,
            autostart: false,
            close_to_tray: true,
            ball_size: default_ball_size(),
            ball_color: default_ball_color(),
        }
    }
}

#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    Json(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "读写本地数据失败: {e}"),
            StoreError::Json(msg) => write!(f, "本地数据格式错误: {msg}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Json(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// 数据存取入口。
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: PathBuf) -> Self {
        Store { root }
    }

    /// 默认存储根：`%APPDATA%/com.wechatbridge.windows`。
    pub fn default_root() -> PathBuf {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir());
        base.join("com.wechatbridge.windows")
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    // ---------- 路径 ----------

    fn settings_path(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    fn scenes_path(&self) -> PathBuf {
        self.root.join("scenes.json")
    }

    fn targets_path(&self) -> PathBuf {
        self.root.join("targets.json")
    }

    pub fn batches_dir(&self) -> PathBuf {
        self.root.join("batches")
    }

    pub fn batch_dir(&self, batch_id: &str) -> PathBuf {
        self.batches_dir().join(batch_id)
    }

    fn manifest_path(&self, batch_id: &str) -> PathBuf {
        self.batch_dir(batch_id).join("manifest.json")
    }

    /// 批次内原始 ZIP 副本的路径。
    pub fn archive_path(&self, manifest: &BatchManifest) -> PathBuf {
        self.root.join(&manifest.relative_path).join("archive.zip")
    }

    // ---------- 设置 ----------

    pub fn load_settings(&self) -> Result<AppSettings> {
        match read_json(&self.settings_path())? {
            Some(s) => Ok(s),
            None => Ok(AppSettings::default()),
        }
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        write_json(&self.settings_path(), settings)
    }

    // ---------- 场景 ----------

    /// 读取场景库；文件缺失时写入并返回内置场景（对齐 macOS 版 makeDefault）。
    pub fn load_scenes(&self) -> Result<SceneSettings> {
        match read_json(&self.scenes_path())? {
            Some(s) => Ok(s),
            None => {
                let settings = default_scene_settings();
                self.save_scenes(&settings)?;
                Ok(settings)
            }
        }
    }

    pub fn save_scenes(&self, settings: &SceneSettings) -> Result<()> {
        write_json(&self.scenes_path(), settings)
    }

    // ---------- 转发目标 ----------

    /// 读取目标列表：内置 9 入口始终存在，用户文件按 id 覆盖字段，自定义目标追加在后。
    pub fn load_targets(&self) -> Result<Vec<TargetApp>> {
        let builtins = builtin_targets();
        let saved: Option<Vec<TargetApp>> = read_json(&self.targets_path())?;
        let Some(saved) = saved else {
            write_json(&self.targets_path(), &builtins)?;
            return Ok(builtins);
        };

        let mut result: Vec<TargetApp> = Vec::with_capacity(builtins.len() + 4);
        for mut b in builtins {
            if let Some(user) = saved.iter().find(|t| t.id == b.id) {
                // 用户可改：显示名/进程/标题/exe/pathOnly/enabled；builtin 标记不可被覆盖。
                b.display_name = user.display_name.clone();
                b.process_names = user.process_names.clone();
                b.window_title_keywords = user.window_title_keywords.clone();
                b.exe_path = user.exe_path.clone();
                b.path_only = user.path_only;
                b.enabled = user.enabled;
            }
            result.push(b);
        }
        for t in saved.into_iter().filter(|t| !t.builtin) {
            if !result.iter().any(|r| r.id == t.id) {
                result.push(t);
            }
        }
        Ok(result)
    }

    pub fn save_targets(&self, targets: &[TargetApp]) -> Result<()> {
        write_json(&self.targets_path(), &targets)
    }

    // ---------- 批次 ----------

    /// 新建批次：复制 ZIP 到 batches/<id>/archive.zip 并写 manifest。
    pub fn create_batch(&self, display_name: &str, src_zip: &Path) -> Result<BatchManifest> {
        let now = chrono::Local::now().naive_local();
        let mut id = now.format("%Y%m%d-%H%M%S").to_string();
        if self.batch_dir(&id).exists() {
            let short = &uuid::Uuid::new_v4().to_string()[..8];
            id = format!("{id}-{short}");
        }
        let dir = self.batch_dir(&id);
        fs::create_dir_all(&dir)?;
        fs::copy(src_zip, dir.join("archive.zip"))?;

        let manifest = BatchManifest {
            id: id.clone(),
            created_at: now,
            action: None,
            display_name: display_name.to_string(),
            relative_path: format!("batches/{id}"),
            state: BatchState::Received,
            chat_name: None,
            scene_name: None,
            error: None,
        };
        self.save_manifest(&manifest)?;
        Ok(manifest)
    }

    pub fn save_manifest(&self, manifest: &BatchManifest) -> Result<()> {
        write_json(&self.manifest_path(&manifest.id), manifest)
    }

    /// 全部批次，按创建时间倒序（新在前）。损坏的 manifest 跳过。
    pub fn load_manifests(&self) -> Result<Vec<BatchManifest>> {
        let mut out = Vec::new();
        let dir = self.batches_dir();
        if dir.is_dir() {
            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                let manifest_path = entry.path().join("manifest.json");
                if manifest_path.is_file() {
                    if let Ok(Some(m)) = read_json::<BatchManifest>(&manifest_path) {
                        out.push(m);
                    }
                }
            }
        }
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(out)
    }

    pub fn load_manifest(&self, batch_id: &str) -> Result<Option<BatchManifest>> {
        read_json(&self.manifest_path(batch_id))
    }

    pub fn delete_batch(&self, batch_id: &str) -> Result<()> {
        let dir = self.batch_dir(batch_id);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }
}

// ---------- 内置定义 ----------

/// 内置 9 入口（SPEC §8 的 Windows 目标映射）。
pub fn builtin_targets() -> Vec<TargetApp> {
    let mk = |id: &str,
              name: &str,
              procs: &[&str],
              titles: &[&str],
              path_only: bool| TargetApp {
        id: id.to_string(),
        display_name: name.to_string(),
        process_names: procs.iter().map(|s| s.to_string()).collect(),
        window_title_keywords: titles.iter().map(|s| s.to_string()).collect(),
        exe_path: None,
        path_only,
        builtin: true,
        enabled: true,
    };
    vec![
        // Codex CLI 运行在终端里：pathOnly，激活终端窗口。
        mk("codex", "Codex", &["Codex.exe"], &["Codex"], true),
        mk("claude", "Claude", &["Claude.exe", "claude.exe"], &["Claude"], false),
        mk("doubao", "豆包", &["Doubao.exe"], &["豆包", "Doubao"], false),
        mk("qwen", "通义千问", &["Qwen.exe"], &["通义千问", "Qwen"], false),
        mk(
            "workBuddy",
            "WorkBuddy",
            &["Trae.exe", "Trae CN.exe"],
            &["Trae", "WorkBuddy"],
            false,
        ),
        mk("weSight", "WeSight", &["WeSight.exe"], &["WeSight"], false),
        // Obsidian 走笔记导出而非粘贴（SPEC §6），pathOnly 无意义。
        mk("obsidian", "Obsidian", &["Obsidian.exe"], &["Obsidian"], false),
        mk("clipboard", "剪贴板", &[], &[], false),
        mk("custom", "自定义应用", &[], &[], false),
    ]
}

/// 终端白名单（SPEC §4：pathOnly 默认开启的进程名，小写比较）。
pub fn terminal_process_names() -> &'static [&'static str] {
    &[
        "windowsterminal.exe",
        "wt.exe",
        "cmd.exe",
        "powershell.exe",
        "pwsh.exe",
        "git-bash.exe",
        "mintty.exe",
    ]
}

/// 内置场景（移植自 macOS 版 starterScenes，含公众号提取/视频读取两个官方场景）。
pub fn default_scene_settings() -> SceneSettings {
    let standard_output_spec = "输出以下六项：1. 要点；2. 结论；3. 待办；4. 风险；5. 负责人；6. 截止时间。没有信息的项目明确写“无”。";
    let mk = |id: &str,
              name: &str,
              summary: &str,
              instruction: &str,
              output_spec: &str,
              keywords: &[&str],
              applicability: &str,
              required_skill_ids: &[&str]| WeChatScene {
        id: id.to_string(),
        name: name.to_string(),
        summary: summary.to_string(),
        instruction: instruction.to_string(),
        output_spec: output_spec.to_string(),
        keywords: keywords.iter().map(|s| s.to_string()).collect(),
        enabled: false,
        package_version: "1.0.0".to_string(),
        author: "微信流".to_string(),
        applicability: applicability.to_string(),
        required_skill_ids: required_skill_ids.iter().map(|s| s.to_string()).collect(),
        compatible_agents: AgentId::ALL.to_vec(),
        is_official: true,
    };
    SceneSettings {
        scenes: vec![
            mk(
                "wechatflow.starter.customer-review",
                "客户复盘",
                "提取客户群里的需求、承诺、风险和下一步。",
                "阅读附件里的聊天记录，聚焦客户需求、业务结论、承诺和下一步推进。",
                standard_output_spec,
                &["客户", "甲方"],
                "适合客户群、售后群和甲方沟通群。",
                &[],
            ),
            mk(
                "wechatflow.starter.project-sync",
                "项目周会",
                "整理项目进展、阻塞、负责人和截止时间。",
                "阅读附件里的聊天记录，整理项目进展、决策、阻塞和待办。",
                standard_output_spec,
                &["项目", "周会"],
                "适合项目群、跨团队协作群和固定周会群。",
                &[],
            ),
            mk(
                "wechatflow.starter.daily-summary",
                "日常摘要",
                "按时间线总结一段群聊，保留关键事实和待办。",
                "阅读附件里的微信聊天记录，按时间线总结重要信息，不要逐条复述。",
                standard_output_spec,
                &[],
                "通用群聊摘要场景。",
                &[],
            ),
            mk(
                "wechatflow.official.article-extract",
                "公众号文章提取",
                "从聊天记录中找出公众号文章，提取正文并整理成 Markdown。",
                "读取附件中的聊天记录，找出公众号文章链接或分享卡片，提取标题、公众号、发布时间、正文和图片，并保留原文链接。",
                "按文章逐篇输出 Markdown：标题、公众号、发布时间、核心摘要、正文、图片、原文链接。无法访问的文章明确标记。",
                &["公众号", "文章"],
                "适合包含公众号文章分享的群聊和收藏群。",
                &["wechatbridge.wechat-article-extract"],
            ),
            mk(
                "wechatflow.official.video-reading",
                "视频信息读取",
                "读取聊天里的视频链接或文件，提炼逐字稿、摘要和关键时间点。",
                "读取附件中的聊天记录，找出视频链接或本地视频文件，提取可获得的逐字稿、摘要、关键结论和时间点，并保留来源。",
                "输出来源、时长、逐字稿或摘要、关键结论、关键时间点和无法读取的部分。",
                &["视频", "抖音", "B站"],
                "适合经常分享视频链接或视频文件的群聊。",
                &["wechatbridge.video-information-reading"],
            ),
        ],
        default_scene_id: None,
        attach_to_forwards: false,
    }
}

// ---------- JSON 读写 ----------

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>> {
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    let value = serde_json::from_str(&text)?;
    Ok(Some(value))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)?;
    // 临时文件 + rename，避免写入中断留下半个 JSON。
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[allow(dead_code)]
fn _naive_now() -> NaiveDateTime {
    chrono::Local::now().naive_local()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (Store, PathBuf) {
        let dir = std::env::temp_dir().join(format!("wcb-store-test-{}", uuid::Uuid::new_v4()));
        (Store::new(dir.clone()), dir)
    }

    #[test]
    fn settings_round_trip() {
        let (store, dir) = temp_store();
        let mut settings = AppSettings::default();
        assert_eq!(settings.paste_mode, PasteMode::FileClipboard);
        settings.obsidian_vault = Some(r"D:\Notes".to_string());
        settings.language = "en".to_string();
        store.save_settings(&settings).unwrap();
        assert_eq!(store.load_settings().unwrap(), settings);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scenes_default_has_five_official() {
        let (store, dir) = temp_store();
        let scenes = store.load_scenes().unwrap();
        assert_eq!(scenes.scenes.len(), 5);
        assert!(scenes.scenes.iter().all(|s| s.is_official));
        assert!(scenes
            .scenes
            .iter()
            .any(|s| s.id == "wechatflow.official.article-extract"
                && s.required_skill_ids == vec!["wechatbridge.wechat-article-extract"]));
        assert!(!scenes.attach_to_forwards);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn targets_merge_user_overrides_keep_builtin_flag() {
        let (store, dir) = temp_store();
        let mut targets = store.load_targets().unwrap();
        assert_eq!(targets.len(), 9);
        // 用户修改 claude：禁用 + 自定义 exe
        let claude = targets.iter_mut().find(|t| t.id == "claude").unwrap();
        claude.enabled = false;
        claude.exe_path = Some(r"D:\Apps\Claude.exe".to_string());
        // 追加一个自定义目标
        targets.push(TargetApp {
            id: "custom-abc".to_string(),
            display_name: "我的应用".to_string(),
            process_names: vec!["myapp.exe".to_string()],
            window_title_keywords: vec![],
            exe_path: None,
            path_only: false,
            builtin: false,
            enabled: true,
        });
        store.save_targets(&targets).unwrap();

        let loaded = store.load_targets().unwrap();
        assert_eq!(loaded.len(), 10);
        let claude = loaded.iter().find(|t| t.id == "claude").unwrap();
        assert!(claude.builtin);
        assert!(!claude.enabled);
        assert_eq!(claude.exe_path.as_deref(), Some(r"D:\Apps\Claude.exe"));
        assert!(loaded.iter().any(|t| t.id == "custom-abc" && !t.builtin));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn batch_lifecycle() {
        let (store, dir) = temp_store();
        fs::create_dir_all(&dir).unwrap();
        // 造一个假 ZIP 源文件
        let src = dir.join("src.zip");
        fs::write(&src, b"PK\x03\x04 fake").unwrap();

        let m1 = store.create_batch("聊天记录.zip", &src).unwrap();
        assert_eq!(m1.state, BatchState::Received);
        assert!(store.archive_path(&m1).is_file());

        let list = store.load_manifests().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, m1.id);

        // 状态流转后保存
        let mut m1 = m1;
        m1.state = BatchState::Delivered;
        m1.action = Some(crate::batch::ShareAction::Claude);
        store.save_manifest(&m1).unwrap();
        let reloaded = store.load_manifest(&m1.id).unwrap().unwrap();
        assert_eq!(reloaded.state, BatchState::Delivered);
        assert_eq!(reloaded.action, Some(crate::batch::ShareAction::Claude));

        store.delete_batch(&m1.id).unwrap();
        assert!(store.load_manifests().unwrap().is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn json_write_is_atomic_and_pretty() {
        let (store, dir) = temp_store();
        store.save_settings(&AppSettings::default()).unwrap();
        let text = fs::read_to_string(dir.join("settings.json")).unwrap();
        assert!(text.contains('\n')); // pretty
        assert!(!dir.join("settings.json.tmp").exists());
        fs::remove_dir_all(&dir).ok();
    }
}
