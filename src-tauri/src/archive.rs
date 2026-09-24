//! 微信「合并转发」ZIP 归档解析。
//!
//! 行为规格移植自 macOS 版 `WeChatNativeArchive.swift` 与 `WeChatForward.swift`：
//! - 只读取 ZIP 中的 `.txt` 条目，优先选择名为 `聊天记录.txt` 的文件
//! - 消息格式：`·发送者昵称\n2024年1月5日 14:30\n消息内容（可多行）`
//! - 去 BOM、归一 `\r\n`；第一条消息必须出现在位置 0，否则视为无效归档
//! - 解压全程防 ZIP slip（拒绝 `..`、绝对路径与非法字符）

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use chrono::NaiveDateTime;
use regex::Regex;

/// 一条聊天消息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptRecord {
    pub sender: String,
    pub date: NaiveDateTime,
    pub text: String,
}

/// 解析后的聊天归档。
#[derive(Debug, Clone)]
pub struct Transcript {
    /// 归一化后的完整文本。
    pub body: String,
    pub records: Vec<TranscriptRecord>,
}

#[derive(Debug)]
pub enum ArchiveError {
    Io(std::io::Error),
    Zip(String),
    InvalidTranscript,
    UnsafeEntry(String),
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveError::Io(e) => write!(f, "读取文件失败: {e}"),
            ArchiveError::Zip(msg) => write!(f, "ZIP 归档无法完整读取: {msg}"),
            ArchiveError::InvalidTranscript => {
                write!(f, "微信导出的文件为空或无法完整读取，文件已保留。")
            }
            ArchiveError::UnsafeEntry(name) => write!(f, "ZIP 包含不安全条目: {name}"),
        }
    }
}

impl std::error::Error for ArchiveError {}

impl From<std::io::Error> for ArchiveError {
    fn from(e: std::io::Error) -> Self {
        ArchiveError::Io(e)
    }
}

impl From<zip::result::ZipError> for ArchiveError {
    fn from(e: zip::result::ZipError) -> Self {
        ArchiveError::Zip(e.to_string())
    }
}

/// 解析聊天 TXT 文本为消息记录。
///
/// 与 Swift 版一致：首个正则匹配必须位于位置 0，否则返回 `InvalidTranscript`。
pub fn parse_transcript(body: &str) -> Result<Vec<TranscriptRecord>, ArchiveError> {
    let body = body
        .replace("\r\n", "\n")
        .trim_start_matches('\u{feff}')
        .to_string();
    // (?m)^·([^\n]+)\n(\d{4}年\d{1,2}月\d{1,2}日 \d{2}:\d{2})\n
    let re = Regex::new(r"(?m)^·([^\n]+)\n(\d{4}年\d{1,2}月\d{1,2}日 \d{2}:\d{2})\n")
        .expect("transcript regex must compile");
    let matches: Vec<_> = re.captures_iter(&body).collect();
    match matches.first() {
        Some(first) if first.get(0).unwrap().start() == 0 => {}
        _ => return Err(ArchiveError::InvalidTranscript),
    }

    let mut records = Vec::with_capacity(matches.len());
    for (index, caps) in matches.iter().enumerate() {
        let whole = caps.get(0).unwrap();
        let sender = caps.get(1).unwrap().as_str().to_string();
        let date_raw = caps.get(2).unwrap().as_str();
        let date = NaiveDateTime::parse_from_str(date_raw, "%Y年%m月%d日 %H:%M")
            .map_err(|_| ArchiveError::InvalidTranscript)?;
        let start = whole.end();
        let end = matches
            .get(index + 1)
            .map(|next| next.get(0).unwrap().start())
            .unwrap_or(body.len());
        let text = body[start..end].trim().to_string();
        records.push(TranscriptRecord { sender, date, text });
    }
    Ok(records)
}

/// 从 ZIP 数据中读取聊天归档。仅读取 `.txt` 条目，优先 `聊天记录.txt`。
pub fn transcript_from_zip(zip_path: &Path) -> Result<Option<Transcript>, ArchiveError> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // 收集全部 .txt 条目名，优先精确名为 聊天记录.txt 的条目。
    let mut txt_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = entry.enclosed_name() else {
            return Err(ArchiveError::UnsafeEntry(entry.name().to_string()));
        };
        let name_str = name.to_string_lossy().to_string();
        if name_str.to_lowercase().ends_with(".txt") {
            txt_names.push(name_str);
        }
    }
    if txt_names.is_empty() {
        return Ok(None);
    }
    txt_names.sort_by_key(|n| {
        let base = Path::new(n)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        if base == "聊天记录.txt" {
            0
        } else {
            1
        }
    });

    let mut entry = archive.by_name(&txt_names[0])?;
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut bytes)?;
    // 微信导出 TXT 为 UTF-8（可能带 BOM）；容错解码。
    let body = String::from_utf8_lossy(&bytes).to_string();
    let records = parse_transcript(&body)?;
    Ok(Some(Transcript { body, records }))
}

/// 校验 ZIP 内相对路径安全（防 ZIP slip）。
fn safe_relative_path(name: &str) -> Result<PathBuf, ArchiveError> {
    let path = Path::new(name);
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Normal(part) => {
                let s = part.to_string_lossy();
                // 拒绝 Windows 非法命名字符与保留名风险最低的做法：拒绝控制字符与冒号。
                if s.chars().any(|c| c < '\u{20}' || c == ':') {
                    return Err(ArchiveError::UnsafeEntry(name.to_string()));
                }
                out.push(part);
            }
            Component::CurDir => {}
            _ => return Err(ArchiveError::UnsafeEntry(name.to_string())),
        }
    }
    if out.as_os_str().is_empty() {
        return Err(ArchiveError::UnsafeEntry(name.to_string()));
    }
    Ok(out)
}

/// 将 ZIP 安全解压到目标目录，返回写出的文件路径列表。
pub fn extract_zip_safe(zip_path: &Path, dest: &Path) -> Result<Vec<PathBuf>, ArchiveError> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    fs::create_dir_all(dest)?;
    let mut written = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let raw_name = entry.name().replace('\\', "/");
        if entry.is_dir() {
            continue;
        }
        // enclosed_name 拒绝 .. 与盘符/根路径；再叠加自定义字符校验。
        let Some(enclosed) = entry.enclosed_name() else {
            return Err(ArchiveError::UnsafeEntry(raw_name));
        };
        let rel = safe_relative_path(&enclosed.to_string_lossy().replace('\\', "/"))?;
        let out_path = dest.join(&rel);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = fs::File::create(&out_path)?;
        std::io::copy(&mut entry, &mut out)?;
        written.push(out_path);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\u{feff}·苍何\n2026年9月20日 14:30\n第一条消息\n带图片[图片](123.jpg)\n·向明\n2026年9月20日 14:31\n回复内容\n多行\n内容\n";

    #[test]
    fn parses_sample_records() {
        let records = parse_transcript(SAMPLE).expect("must parse");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].sender, "苍何");
        assert_eq!(
            records[0].date,
            NaiveDateTime::parse_from_str("2026-09-20 14:30", "%Y-%m-%d %H:%M").unwrap()
        );
        assert!(records[0].text.contains("第一条消息"));
        assert!(records[0].text.contains("[图片](123.jpg)"));
        assert_eq!(records[1].sender, "向明");
        assert_eq!(records[1].text, "回复内容\n多行\n内容");
    }

    #[test]
    fn rejects_body_not_starting_with_record() {
        assert!(matches!(
            parse_transcript("前言\n·苍何\n2026年9月20日 14:30\n内容\n"),
            Err(ArchiveError::InvalidTranscript)
        ));
        assert!(matches!(
            parse_transcript(""),
            Err(ArchiveError::InvalidTranscript)
        ));
    }

    #[test]
    fn handles_crlf_and_bom() {
        let body = "·A\r\n2026年1月5日 08:05\r\n你好\r\n";
        let records = parse_transcript(body).expect("must parse");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].text, "你好");
        assert_eq!(records[0].date.month(), 1);
        assert_eq!(records[0].date.day(), 5);
    }

    use chrono::Datelike;

    #[test]
    fn rejects_unsafe_entry_path() {
        assert!(safe_relative_path("../evil.txt").is_err());
        assert!(safe_relative_path("C:/evil.txt").is_err());
        assert!(safe_relative_path("a/../b.txt").is_err());
        assert!(safe_relative_path("dir/ok.txt").is_ok());
    }
}
