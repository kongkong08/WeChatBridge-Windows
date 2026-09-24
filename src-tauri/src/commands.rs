//! Tauri commands：前端契约桥接。
//!
//! 契约 = `src/api.ts`，DTO 字段 camelCase 对齐 `src/types.ts`。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;

use crate::archive;
use crate::batch::{BatchManifest, BatchState, ShareAction};
use crate::obsidian;
use crate::paste_plan::{self, PastePayload};
use crate::scene::SceneSettings;
use crate::store::{AppSettings, PasteMode, Store, TargetApp};
#[cfg(windows)]
use crate::win32;

/// 全局状态：本地数据层。
pub struct AppState {
    pub store: Store,
}

// ---------- DTO ----------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptRecordDto {
    pub sender: String,
    pub date: String,
    pub text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub chat_name: Option<String>,
    pub message_count: usize,
    pub records: Vec<TranscriptRecordDto>,
    pub attachments: Vec<String>,
    pub transcript_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetStatus {
    #[serde(flatten)]
    pub app: TargetApp,
    pub running: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardResult {
    pub manifest: BatchManifest,
    pub used_fallback_copy: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub official: bool,
}

// ---------- 批次 ----------

#[tauri::command]
pub fn import_zip(state: State<'_, AppState>, path: String) -> Result<BatchManifest, String> {
    let src = PathBuf::from(&path);
    if !src.is_file() {
        return Err(format!("文件不存在：{path}"));
    }
    let is_zip = src
        .extension()
        .map(|e| e.to_string_lossy().eq_ignore_ascii_case("zip"))
        .unwrap_or(false);
    if !is_zip {
        return Err("请选择微信导出的 ZIP 文件".to_string());
    }
    let display_name = src
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "聊天记录.zip".to_string());

    let mut manifest = state
        .store
        .create_batch(&display_name, &src)
        .map_err(|e| e.to_string())?;

    // 立即安全解压到批次目录，供预览 / 转发 / Obsidian 复用。
    let archive_path = state.store.archive_path(&manifest);
    if let Err(e) = ensure_extracted(&state.store, &manifest) {
        state.store.delete_batch(&manifest.id).ok();
        return Err(e.to_string());
    }
    // 解析聊天文本，推断聊天名（文件名去扩展名）。
    match archive::transcript_from_zip(&archive_path) {
        Ok(Some(_)) => {
            manifest.chat_name = src
                .file_stem()
                .map(|s| s.to_string_lossy().to_string());
            state
                .store
                .save_manifest(&manifest)
                .map_err(|e| e.to_string())?;
        }
        Ok(None) => {}
        Err(e) => {
            // 无有效聊天文本：批次保留（SPEC：文件已保留），仅记录错误。
            manifest.error = Some(e.to_string());
            state
                .store
                .save_manifest(&manifest)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(manifest)
}

#[tauri::command]
pub fn list_batches(state: State<'_, AppState>) -> Result<Vec<BatchManifest>, String> {
    state.store.load_manifests().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_batch(state: State<'_, AppState>, batch_id: String) -> Result<(), String> {
    state.store.delete_batch(&batch_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn preview_batch(state: State<'_, AppState>, batch_id: String) -> Result<Preview, String> {
    let manifest = load_manifest(&state.store, &batch_id)?;
    let archive_path = state.store.archive_path(&manifest);
    let transcript = archive::transcript_from_zip(&archive_path)
        .ok()
        .flatten();

    let files_dir = extracted_dir(&state.store, &manifest);
    let mut attachments = Vec::new();
    let mut transcript_path = None;
    if files_dir.is_dir() {
        collect_files(&files_dir, &files_dir, &mut |rel, abs| {
            if rel.to_lowercase().ends_with(".txt") {
                if transcript_path.is_none() {
                    transcript_path = Some(abs.to_string_lossy().to_string());
                }
            } else {
                attachments.push(rel);
            }
        });
    }
    attachments.sort();

    let (message_count, records) = match &transcript {
        Some(t) => (
            t.records.len(),
            t.records
                .iter()
                .map(|r| TranscriptRecordDto {
                    sender: r.sender.clone(),
                    date: r.date.format("%Y-%m-%dT%H:%M:%S").to_string(),
                    text: r.text.clone(),
                })
                .collect(),
        ),
        None => (0, Vec::new()),
    };

    Ok(Preview {
        chat_name: manifest.chat_name.clone(),
        message_count,
        records,
        attachments,
        transcript_path,
    })
}

#[tauri::command]
pub fn open_batch_folder(state: State<'_, AppState>, batch_id: String) -> Result<(), String> {
    let manifest = load_manifest(&state.store, &batch_id)?;
    let dir = state.store.batch_dir(&manifest.id);
    open_in_file_manager(&dir)
}

// ---------- 转发 ----------

#[tauri::command]
pub fn list_targets(state: State<'_, AppState>) -> Result<Vec<TargetStatus>, String> {
    let targets = state.store.load_targets().map_err(|e| e.to_string())?;
    Ok(targets
        .into_iter()
        .map(|app| {
            let running = if app.id == "clipboard" {
                true
            } else if app.process_names.is_empty() {
                false
            } else {
                #[cfg(windows)]
                {
                    win32::is_running(&app.process_names)
                }
                #[cfg(not(windows))]
                {
                    false
                }
            };
            TargetStatus { app, running }
        })
        .collect())
}

#[tauri::command]
pub async fn forward_batch(
    state: State<'_, AppState>,
    batch_id: String,
    target_id: String,
    scene_id: Option<String>,
) -> Result<ForwardResult, String> {
    let store = &state.store;
    let mut manifest = load_manifest(store, &batch_id)?;

    // Obsidian 走笔记导出（SPEC §6），不走粘贴。
    if target_id == "obsidian" {
        return match export_obsidian_inner(store, &mut manifest, scene_id.as_deref()) {
            Ok(note_path) => Ok(ForwardResult {
                manifest,
                used_fallback_copy: false,
                message: format!("已导出到 Obsidian：{note_path}"),
            }),
            Err(e) => {
                mark_failed(store, &mut manifest, &e);
                Ok(ForwardResult {
                    manifest,
                    used_fallback_copy: false,
                    message: e,
                })
            }
        };
    }

    let targets = store.load_targets().map_err(|e| e.to_string())?;
    let Some(target) = targets.iter().find(|t| t.id == target_id).cloned() else {
        return Err(format!("未知转发目标：{target_id}"));
    };
    if !target.enabled {
        return Err(format!("目标「{}」已禁用", target.display_name));
    }

    let settings = store.load_settings().map_err(|e| e.to_string())?;
    let plan = match build_plan(store, &manifest, &target, &settings, scene_id.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            mark_failed(store, &mut manifest, &e);
            return Ok(ForwardResult {
                manifest,
                used_fallback_copy: false,
                message: e,
            });
        }
    };

    // 剪贴板目标：直接复制 manual_payload。
    if target.id == "clipboard" {
        let payload = paste_plan::manual_payload(&plan)
            .cloned()
            .ok_or_else(|| "粘贴计划为空".to_string())?;
        return match copy_payload(&payload, settings.paste_mode) {
            Ok(what) => {
                manifest.state = BatchState::Copied;
                manifest.action = Some(ShareAction::Clipboard);
                manifest.scene_name = scene_name_of(store, scene_id.as_deref());
                manifest.error = None;
                store.save_manifest(&manifest).map_err(|e| e.to_string())?;
                Ok(ForwardResult {
                    manifest,
                    used_fallback_copy: false,
                    message: format!("已复制到剪贴板：{what}，请到目标窗口按 Ctrl+V"),
                })
            }
            Err(e) => {
                mark_failed(store, &mut manifest, &e);
                Ok(ForwardResult {
                    manifest,
                    used_fallback_copy: false,
                    message: e,
                })
            }
        };
    }

    // 应用目标：激活窗口 + 自动粘贴；失败兜底复制到剪贴板（SPEC §5）。
    #[cfg(windows)]
    {
        let outcome = deliver_to_window(&target, &plan, settings.paste_mode);
        match outcome {
            Ok(()) => {
                manifest.state = BatchState::Delivered;
                manifest.action = Some(action_of(&target.id));
                manifest.scene_name = scene_name_of(store, scene_id.as_deref());
                manifest.error = None;
                store.save_manifest(&manifest).map_err(|e| e.to_string())?;
                Ok(ForwardResult {
                    manifest,
                    used_fallback_copy: false,
                    message: format!("已送达「{}」", target.display_name),
                })
            }
            Err(reason) => {
                // 兜底：复制 manual_payload，由用户手动 Ctrl+V。
                let payload = paste_plan::manual_payload(&plan).cloned();
                let copied = payload
                    .as_ref()
                    .and_then(|p| copy_payload(p, settings.paste_mode).ok());
                match copied {
                    Some(what) => {
                        manifest.state = BatchState::Copied;
                        manifest.action = Some(action_of(&target.id));
                        manifest.scene_name = scene_name_of(store, scene_id.as_deref());
                        manifest.error = Some(reason.clone());
                        store.save_manifest(&manifest).map_err(|e| e.to_string())?;
                        Ok(ForwardResult {
                            manifest,
                            used_fallback_copy: true,
                            message: format!(
                                "未能自动粘贴到「{}」（{reason}）。已复制{what}到剪贴板，请手动 Ctrl+V。",
                                target.display_name
                            ),
                        })
                    }
                    None => {
                        mark_failed(store, &mut manifest, &reason);
                        Ok(ForwardResult {
                            manifest,
                            used_fallback_copy: false,
                            message: format!("转发失败：{reason}"),
                        })
                    }
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (plan, settings);
        Err("自动粘贴仅支持 Windows".to_string())
    }
}

#[tauri::command]
pub fn copy_manual_payload(
    state: State<'_, AppState>,
    batch_id: String,
    target_id: String,
    scene_id: Option<String>,
) -> Result<String, String> {
    let store = &state.store;
    let mut manifest = load_manifest(store, &batch_id)?;
    let targets = store.load_targets().map_err(|e| e.to_string())?;
    let target = targets
        .iter()
        .find(|t| t.id == target_id)
        .cloned()
        .unwrap_or(TargetApp {
            id: "clipboard".to_string(),
            display_name: "剪贴板".to_string(),
            process_names: vec![],
            window_title_keywords: vec![],
            exe_path: None,
            path_only: false,
            builtin: true,
            enabled: true,
        });
    let settings = store.load_settings().map_err(|e| e.to_string())?;
    let plan = build_plan(store, &manifest, &target, &settings, scene_id.as_deref())?;
    let payload = paste_plan::manual_payload(&plan)
        .cloned()
        .ok_or_else(|| "粘贴计划为空".to_string())?;
    let what = copy_payload(&payload, settings.paste_mode)?;
    manifest.state = BatchState::Copied;
    manifest.action = Some(action_of(&target.id));
    manifest.scene_name = scene_name_of(store, scene_id.as_deref());
    manifest.error = None;
    store.save_manifest(&manifest).map_err(|e| e.to_string())?;
    Ok(what)
}

#[tauri::command]
pub fn export_obsidian(state: State<'_, AppState>, batch_id: String) -> Result<String, String> {
    let store = &state.store;
    let mut manifest = load_manifest(store, &batch_id)?;
    let note_path = export_obsidian_inner(store, &mut manifest, None)?;
    Ok(note_path)
}

// ---------- 场景 ----------

#[tauri::command]
pub fn list_scenes(state: State<'_, AppState>) -> Result<SceneSettings, String> {
    state.store.load_scenes().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_scenes(state: State<'_, AppState>, settings: SceneSettings) -> Result<(), String> {
    state.store.save_scenes(&settings).map_err(|e| e.to_string())
}

// ---------- 设置 ----------

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    state.store.load_settings().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<(), String> {
    state.store.save_settings(&settings).map_err(|e| e.to_string())?;
    // 应用开机自启（Windows 注册表 Run 键）。
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if settings.autostart {
        manager.enable().map_err(|e| e.to_string())?;
    } else {
        manager.disable().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn save_targets(state: State<'_, AppState>, targets: Vec<TargetApp>) -> Result<(), String> {
    state.store.save_targets(&targets).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pick_file(
    app: tauri::AppHandle,
    filters: Vec<(String, Vec<String>)>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let mut builder = app.dialog().file();
    for (name, exts) in filters {
        let exts: Vec<&str> = exts.iter().map(String::as_str).collect();
        builder = builder.add_filter(name, &exts);
    }
    Ok(builder
        .blocking_pick_file()
        .and_then(|p| p.as_path().map(|x| x.to_string_lossy().to_string())))
}

#[tauri::command]
pub fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    Ok(app
        .dialog()
        .file()
        .blocking_pick_folder()
        .and_then(|p| p.as_path().map(|x| x.to_string_lossy().to_string())))
}

// ---------- 技能中心 ----------

/// 内置官方技能（对应官方场景的 required_skill_ids）。
fn builtin_skills() -> Vec<SkillInfo> {
    vec![
        SkillInfo {
            id: "wechatbridge.wechat-article-extract".to_string(),
            name: "公众号文章提取".to_string(),
            description: "从聊天记录中提取公众号文章正文并整理为 Markdown".to_string(),
            official: true,
        },
        SkillInfo {
            id: "wechatbridge.video-information-reading".to_string(),
            name: "视频信息读取".to_string(),
            description: "读取聊天中的视频链接或文件，提炼逐字稿、摘要与关键时间点".to_string(),
            official: true,
        },
    ]
}

fn skills_dir(store: &Store) -> PathBuf {
    store.root().join("skills")
}

#[tauri::command]
pub fn list_skills(state: State<'_, AppState>) -> Result<Vec<SkillInfo>, String> {
    let mut skills = builtin_skills();
    let dir = skills_dir(&state.store);
    if dir.is_dir() {
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                if let Some(info) = parse_skill_file(&path) {
                    if !skills.iter().any(|s| s.id == info.id) {
                        skills.push(info);
                    }
                }
            }
        }
    }
    Ok(skills)
}

#[tauri::command]
pub fn install_skill(state: State<'_, AppState>, path: String) -> Result<SkillInfo, String> {
    let src = PathBuf::from(&path);
    let mut info = parse_skill_file(&src)
        .ok_or_else(|| "SKILL 文件缺少有效 frontmatter（需含 id/name/description）".to_string())?;
    if builtin_skills().iter().any(|s| s.id == info.id) {
        return Err("该技能为内置技能，无需安装".to_string());
    }
    info.official = false;
    let dir = skills_dir(&state.store);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(format!("{}.md", sanitize_filename(&info.id)));
    fs::copy(&src, &dest).map_err(|e| e.to_string())?;
    Ok(info)
}

#[tauri::command]
pub fn remove_skill(state: State<'_, AppState>, skill_id: String) -> Result<(), String> {
    if builtin_skills().iter().any(|s| s.id == skill_id) {
        return Err("内置技能不可删除".to_string());
    }
    let path = skills_dir(&state.store).join(format!("{}.md", sanitize_filename(&skill_id)));
    if path.is_file() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ---------- 内部辅助 ----------

fn load_manifest(store: &Store, batch_id: &str) -> Result<BatchManifest, String> {
    store
        .load_manifest(batch_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("批次不存在：{batch_id}"))
}

fn extracted_dir(store: &Store, manifest: &BatchManifest) -> PathBuf {
    store.batch_dir(&manifest.id).join("files")
}

/// 确保 ZIP 已安全解压到批次 files/ 目录。
fn ensure_extracted(store: &Store, manifest: &BatchManifest) -> Result<PathBuf, archive::ArchiveError> {
    let dir = extracted_dir(store, manifest);
    if !dir.is_dir() {
        archive::extract_zip_safe(&store.archive_path(manifest), &dir)?;
    }
    Ok(dir)
}

/// 递归收集文件，回调相对路径（/ 分隔）与绝对路径。
fn collect_files(root: &Path, dir: &Path, f: &mut impl FnMut(String, PathBuf)) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, f);
        } else if let Ok(rel) = path.strip_prefix(root) {
            f(rel.to_string_lossy().replace('\\', "/"), path);
        }
    }
}

/// 组装粘贴计划：URL 列表 = 原始 ZIP + 解压附件目录（如有附件）。
fn build_plan(
    store: &Store,
    manifest: &BatchManifest,
    target: &TargetApp,
    _settings: &AppSettings,
    scene_id: Option<&str>,
) -> Result<Vec<PastePayload>, String> {
    let mut urls = vec![store.archive_path(manifest)];
    let files_dir = extracted_dir(store, manifest);
    if files_dir.is_dir() {
        let mut non_txt = 0;
        collect_files(&files_dir, &files_dir, &mut |rel, _| {
            if !rel.to_lowercase().ends_with(".txt") {
                non_txt += 1;
            }
        });
        if non_txt > 0 {
            urls.push(files_dir);
        }
    }
    let prompt = resolve_prompt(store, scene_id);
    Ok(paste_plan::make(&urls, target.path_only, prompt.as_deref()))
}

/// 场景提示词：UI 指定场景优先，否则「转发时附带场景」总开关 + 默认场景。
fn resolve_prompt(store: &Store, scene_id: Option<&str>) -> Option<String> {
    let scenes = store.load_scenes().ok()?;
    let pick = |id: &str| scenes.scenes.iter().find(|s| s.id == id && s.enabled);
    let scene = scene_id
        .and_then(|id| pick(id))
        .or_else(|| {
            if scenes.attach_to_forwards {
                scenes.default_scene_id.as_deref().and_then(|id| pick(id))
            } else {
                None
            }
        })?;
    let prompt = format!("{}\n{}", scene.instruction.trim(), scene.output_spec.trim())
        .trim()
        .to_string();
    if prompt.is_empty() {
        None
    } else {
        Some(prompt)
    }
}

fn scene_name_of(store: &Store, scene_id: Option<&str>) -> Option<String> {
    let id = scene_id?;
    store
        .load_scenes()
        .ok()?
        .scenes
        .into_iter()
        .find(|s| s.id == id)
        .map(|s| s.name)
}

fn action_of(target_id: &str) -> ShareAction {
    match target_id {
        "codex" => ShareAction::Codex,
        "claude" => ShareAction::Claude,
        "doubao" => ShareAction::Doubao,
        "qwen" => ShareAction::Qwen,
        "workBuddy" => ShareAction::WorkBuddy,
        "weSight" => ShareAction::WeSight,
        "obsidian" => ShareAction::Obsidian,
        "clipboard" => ShareAction::Clipboard,
        _ => ShareAction::Custom,
    }
}

fn mark_failed(store: &Store, manifest: &mut BatchManifest, reason: &str) {
    manifest.state = BatchState::Failed;
    manifest.error = Some(reason.to_string());
    let _ = store.save_manifest(manifest);
}

/// 复制一个 payload 到剪贴板，返回内容描述。
fn copy_payload(payload: &PastePayload, mode: PasteMode) -> Result<String, String> {
    #[cfg(windows)]
    {
        match payload {
            PastePayload::Text(t) => {
                win32::set_clipboard_text(t).map_err(|e| e.to_string())?;
                Ok("提示词与路径文本".to_string())
            }
            PastePayload::Files(paths) => match mode {
                PasteMode::FileClipboard => {
                    win32::set_clipboard_files(paths).map_err(|e| e.to_string())?;
                    Ok(format!("{} 个文件", paths.len()))
                }
                PasteMode::TextWithPaths => {
                    win32::set_clipboard_text(&paste_plan::shell_line(paths))
                        .map_err(|e| e.to_string())?;
                    Ok("文件路径文本".to_string())
                }
            },
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (payload, mode);
        Err("剪贴板操作仅支持 Windows".to_string())
    }
}

/// 激活目标窗口并按计划逐条粘贴。
#[cfg(windows)]
fn deliver_to_window(
    target: &TargetApp,
    plan: &[PastePayload],
    mode: PasteMode,
) -> Result<(), String> {
    let mut window = win32::find_window(&target.process_names, &target.window_title_keywords);
    if window.is_none() {
        if let Some(exe) = target.exe_path.as_deref() {
            let exe_path = Path::new(exe);
            if exe_path.is_file() {
                win32::launch_exe(exe_path).map_err(|e| e.to_string())?;
                std::thread::sleep(std::time::Duration::from_millis(1500));
                window = win32::find_window(&target.process_names, &target.window_title_keywords);
            }
        }
    }
    let Some(window) = window else {
        return Err("目标应用未运行且未找到窗口".to_string());
    };

    for payload in plan {
        match payload {
            PastePayload::Text(t) => win32::set_clipboard_text(t).map_err(|e| e.to_string())?,
            PastePayload::Files(paths) => match mode {
                PasteMode::FileClipboard => {
                    win32::set_clipboard_files(paths).map_err(|e| e.to_string())?
                }
                PasteMode::TextWithPaths => {
                    win32::set_clipboard_text(&paste_plan::shell_line(paths))
                        .map_err(|e| e.to_string())?
                }
            },
        }
        win32::activate_window(window.hwnd).map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(250));
        win32::send_ctrl_v().map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
    Ok(())
}

/// Obsidian 导出：渲染笔记 + 复制附件，回写批次状态，返回笔记路径。
fn export_obsidian_inner(
    store: &Store,
    manifest: &mut BatchManifest,
    scene_id: Option<&str>,
) -> Result<String, String> {
    let settings = store.load_settings().map_err(|e| e.to_string())?;
    let vault = settings
        .obsidian_vault
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "请先在设置中选择 Obsidian 仓库目录".to_string())?;

    let archive_path = store.archive_path(manifest);
    let transcript = archive::transcript_from_zip(&archive_path)
        .map_err(|e| e.to_string())?;
    let title = obsidian::title(
        manifest.chat_name.as_deref(),
        transcript.as_ref(),
        &manifest.display_name,
    );
    let scene_name = scene_name_of(store, scene_id);

    // 笔记目录：<vault>/微信流/<标题>-<批次id>/，附件放 附件/。
    let note_dir = Path::new(vault)
        .join("微信流")
        .join(format!("{}-{}", sanitize_filename(&title), manifest.id));
    let attach_dir = note_dir.join("附件");
    fs::create_dir_all(&attach_dir).map_err(|e| e.to_string())?;

    // 复制原始 ZIP 与解压附件。
    fs::copy(&archive_path, attach_dir.join(&manifest.display_name))
        .map_err(|e| e.to_string())?;
    let mut attachments: HashMap<String, String> = HashMap::new();
    let files_dir = extracted_dir(store, manifest);
    if files_dir.is_dir() {
        collect_files(&files_dir, &files_dir, &mut |rel, abs| {
            if rel.to_lowercase().ends_with(".txt") {
                return;
            }
            let base = Path::new(&rel)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| rel.clone());
            // 重名时保留目录前缀避免覆盖。
            let saved = if attachments.contains_key(&base) {
                rel.replace('/', "_")
            } else {
                base.clone()
            };
            if fs::copy(&abs, attach_dir.join(&saved)).is_ok() {
                attachments.entry(base).or_insert(saved);
            }
        });
    }

    let note = obsidian::render(
        &title,
        manifest.chat_name.as_deref(),
        scene_name.as_deref(),
        &manifest.created_at,
        transcript.as_ref(),
        &manifest.display_name,
        &attachments,
    );
    let note_path = note_dir.join(format!("{}.md", sanitize_filename(&title)));
    fs::write(&note_path, note).map_err(|e| e.to_string())?;

    manifest.state = BatchState::Delivered;
    manifest.action = Some(ShareAction::Obsidian);
    manifest.scene_name = scene_name;
    manifest.error = None;
    store.save_manifest(manifest).map_err(|e| e.to_string())?;
    Ok(note_path.to_string_lossy().to_string())
}

/// Windows 文件名非法字符替换。
fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c < '\u{20}' => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches('.').trim();
    if trimmed.is_empty() {
        "未命名".to_string()
    } else {
        trimmed.chars().take(80).collect()
    }
}

/// 极简 SKILL.md frontmatter 解析（id/name/description）。
fn parse_skill_file(path: &Path) -> Option<SkillInfo> {
    let text = fs::read_to_string(path).ok()?;
    let text = text.trim_start_matches('\u{feff}');
    if !text.starts_with("---") {
        return None;
    }
    let rest = &text[3..];
    let end = rest.find("\n---")?;
    let mut id = None;
    let mut name = None;
    let mut description = None;
    for line in rest[..end].lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'').to_string();
        match key.trim() {
            "id" => id = Some(value),
            "name" => name = Some(value),
            "description" => description = Some(value),
            _ => {}
        }
    }
    Some(SkillInfo {
        id: id?,
        name: name?,
        description: description.unwrap_or_default(),
        official: false,
    })
}

fn open_in_file_manager(dir: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(dir)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("打开文件夹失败: {e}"))
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("open")
            .arg(dir)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("打开文件夹失败: {e}"))
    }
}
