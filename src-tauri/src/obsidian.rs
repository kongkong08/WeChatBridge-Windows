//! Obsidian 归档笔记生成。
//!
//! 行为规格移植自 macOS 版 `ObsidianNote.swift`：
//! - frontmatter：title/source/chat?/scene?/exported/messages?/archive
//! - 标题：优先聊天名（自动补「的聊天」后缀）；回退参与者规则；再回退归档文件名
//! - 消息渲染：`**发送者** · yyyy-MM-dd HH:mm` + 空行 + 正文 + 附件链接
//! - 附件：图片/视频/音频/PDF 用 `![[...]]` 嵌入，其余 `[[...]]`；转义 `\ [ ] # ^ |`

use std::collections::HashMap;

use chrono::NaiveDateTime;

use crate::archive::Transcript;

/// 生成笔记标题。
pub fn title(chat_name: Option<&str>, transcript: Option<&Transcript>, archive_name: &str) -> String {
    if let Some(name) = usable(chat_name) {
        return if name.ends_with("的聊天") {
            name.to_string()
        } else {
            format!("{name}的聊天")
        };
    }
    let participants = ordered_participants(transcript);
    match participants.len() {
        0 => {}
        1 => return format!("{}的聊天", participants[0]),
        2 => return format!("{}与{}的聊天", participants[0], participants[1]),
        n => return format!("{}等{n}人的聊天", participants[0]),
    }
    std::path::Path::new(archive_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| archive_name.to_string())
}

/// 渲染完整 Markdown 笔记。
///
/// `attachments`：原始文件名 → 保存后的文件名（保存在 `附件/` 下）。
pub fn render(
    note_title: &str,
    chat_name: Option<&str>,
    scene_name: Option<&str>,
    created_at: &NaiveDateTime,
    transcript: Option<&Transcript>,
    archive_name: &str,
    attachments: &HashMap<String, String>,
) -> String {
    let exported = created_at.format("%Y-%m-%d %H:%M");

    let mut yaml = vec![
        "---".to_string(),
        format!("title: {}", quoted(note_title)),
        "source: WeChat".to_string(),
    ];
    if let Some(c) = usable(chat_name) {
        yaml.push(format!("chat: {}", quoted(c)));
    }
    if let Some(s) = usable(scene_name) {
        yaml.push(format!("scene: {}", quoted(s)));
    }
    yaml.push(format!("exported: {exported}"));
    if let Some(t) = transcript {
        yaml.push(format!("messages: {}", t.records.len()));
    }
    yaml.push(format!("archive: {}", quoted(&format!("附件/{archive_name}"))));
    yaml.push("---".to_string());

    let mut lines = yaml;
    lines.push(String::new());
    lines.push(format!("# {note_title}"));
    lines.push(String::new());
    lines.push(format!("> 来源：微信 · 原始归档：[[附件/{archive_name}]]"));

    match transcript {
        Some(t) if !t.records.is_empty() => {
            lines.push(String::new());
            lines.push("## 聊天记录".to_string());
            for record in &t.records {
                lines.push(String::new());
                lines.push(format!(
                    "**{}** · {}",
                    record.sender,
                    record.date.format("%Y-%m-%d %H:%M")
                ));
                lines.push(String::new());
                lines.push(record.text.clone());
                for attachment in referenced_attachments(&record.text, attachments) {
                    lines.push(String::new());
                    lines.push(attachment_link(&attachment));
                }
            }
        }
        Some(t) => {
            lines.push(String::new());
            lines.push("## 聊天记录".to_string());
            lines.push(String::new());
            lines.push(t.body.clone());
        }
        None => {
            lines.push(String::new());
            lines.push("未能从原始归档中解析聊天文本。原始 ZIP 已保留，可在附件中打开。".to_string());
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

/// 在消息正文中查找被引用的附件名，按名字长度降序消耗（与 Swift 版一致）。
fn referenced_attachments(text: &str, available: &HashMap<String, String>) -> Vec<String> {
    let mut remaining = text.to_string();
    let mut result = Vec::new();
    let mut names: Vec<&String> = available.keys().collect();
    names.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)).reverse());
    for name in names {
        if name.is_empty() || !remaining.contains(name.as_str()) {
            continue;
        }
        if let Some(saved) = available.get(name) {
            result.push(saved.clone());
            remaining = remaining.replace(name.as_str(), "");
        }
    }
    result
}

/// Obsidian 附件链接。图片/视频/音频/PDF 嵌入显示，其余普通链接。
fn attachment_link(name: &str) -> String {
    let path = format!("附件/{name}")
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('#', "\\#")
        .replace('^', "\\^")
        .replace('|', "\\|");
    if is_embeddable(name) {
        format!("![[{path}]]")
    } else {
        format!("[[{path}]]")
    }
}

fn is_embeddable(name: &str) -> bool {
    let ext = name
        .rsplit('.')
        .next()
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    matches!(
        ext.as_str(),
        // 图片
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "heic" | "heif" | "bmp" | "svg" | "tiff" | "tif"
        // 视频
        | "mp4" | "mov" | "m4v" | "avi" | "mkv" | "webm" | "3gp"
        // 音频
        | "mp3" | "m4a" | "aac" | "wav" | "flac" | "ogg" | "amr"
        // PDF
        | "pdf"
    )
}

/// YAML 双引号字符串转义。
fn quoted(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!("\"{escaped}\"")
}

fn ordered_participants(transcript: Option<&Transcript>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    transcript
        .map(|t| {
            t.records
                .iter()
                .filter_map(|r| {
                    let sender = r.sender.trim();
                    if !sender.is_empty() && seen.insert(sender.to_string()) {
                        Some(sender.to_string())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn usable(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{Transcript, TranscriptRecord};

    fn dt() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-09-20 14:30", "%Y-%m-%d %H:%M").unwrap()
    }

    fn transcript() -> Transcript {
        Transcript {
            body: String::new(),
            records: vec![
                TranscriptRecord {
                    sender: "苍何".to_string(),
                    date: dt(),
                    text: "看这张图 [图片](123.jpg)".to_string(),
                },
                TranscriptRecord {
                    sender: "向明".to_string(),
                    date: dt(),
                    text: "收到".to_string(),
                },
            ],
        }
    }

    #[test]
    fn title_prefers_chat_name_and_appends_suffix() {
        assert_eq!(title(Some("产品讨论组"), None, "x.zip"), "产品讨论组的聊天");
        assert_eq!(title(Some("产品讨论组的聊天"), None, "x.zip"), "产品讨论组的聊天");
    }

    #[test]
    fn title_falls_back_to_participants() {
        assert_eq!(title(None, Some(&transcript()), "x.zip"), "苍何与向明的聊天");
        assert_eq!(title(Some("  "), Some(&transcript()), "x.zip"), "苍何与向明的聊天");
        assert_eq!(title(None, None, "聊天记录 2026.zip"), "聊天记录 2026");
    }

    #[test]
    fn renders_frontmatter_and_records() {
        let mut attachments = HashMap::new();
        attachments.insert("123.jpg".to_string(), "123.jpg".to_string());
        let note = render(
            "产品讨论组的聊天",
            Some("产品讨论组"),
            Some("群聊总结"),
            &dt(),
            Some(&transcript()),
            "聊天记录 2026.zip",
            &attachments,
        );
        assert!(note.starts_with("---\ntitle: \"产品讨论组的聊天\""));
        assert!(note.contains("chat: \"产品讨论组\""));
        assert!(note.contains("scene: \"群聊总结\""));
        assert!(note.contains("exported: 2026-09-20 14:30"));
        assert!(note.contains("messages: 2"));
        assert!(note.contains("archive: \"附件/聊天记录 2026.zip\""));
        assert!(note.contains("# 产品讨论组的聊天"));
        assert!(note.contains("**苍何** · 2026-09-20 14:30"));
        assert!(note.contains("![[附件/123.jpg]]"));
    }

    #[test]
    fn attachment_link_embeds_media_only() {
        assert_eq!(attachment_link("a.png"), "![[附件/a.png]]");
        assert_eq!(attachment_link("b.pdf"), "![[附件/b.pdf]]");
        assert_eq!(attachment_link("c.docx"), "[[附件/c.docx]]");
        assert_eq!(attachment_link("d[1].png"), "![[附件/d\\[1\\].png]]");
    }

    #[test]
    fn renders_fallback_when_no_transcript() {
        let note = render("X", None, None, &dt(), None, "a.zip", &HashMap::new());
        assert!(note.contains("未能从原始归档中解析聊天文本"));
        assert!(!note.contains("messages:"));
    }
}
