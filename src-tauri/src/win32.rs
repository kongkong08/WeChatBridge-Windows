//! Windows 系统集成：窗口枚举/激活、剪贴板文本/文件、SendInput 模拟 Ctrl+V。
//!
//! 对应 macOS 版的 NSRunningApplication 激活 + NSPasteboard（SPEC §5）。
//! 仅编译于 Windows；本模块所有 API 均为应用级，无需辅助功能权限。

#![cfg(windows)]

use std::path::{Path, PathBuf};

use windows::core::Result as WinResult;
use windows::Win32::Foundation::{BOOL, CloseHandle, HWND, LPARAM, POINT, WPARAM};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Ole::{CF_HDROP, CF_UNICODETEXT};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
    TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
};
use windows::Win32::UI::Shell::DROPFILES;
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible, SetForegroundWindow, ShowWindow, SW_RESTORE,
};

#[derive(Debug)]
pub enum Win32Error {
    Api(String),
    ClipboardBusy,
    InvalidInput(String),
}

impl std::fmt::Display for Win32Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Win32Error::Api(msg) => write!(f, "Windows API 调用失败: {msg}"),
            Win32Error::ClipboardBusy => write!(f, "剪贴板正被其他应用占用"),
            Win32Error::InvalidInput(msg) => write!(f, "输入无效: {msg}"),
        }
    }
}

impl std::error::Error for Win32Error {}

impl From<windows::core::Error> for Win32Error {
    fn from(e: windows::core::Error) -> Self {
        Win32Error::Api(e.message())
    }
}

pub type Result<T> = std::result::Result<T, Win32Error>;

/// 一个可见顶层窗口。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub pid: u32,
    pub title: String,
    /// 进程映像文件名（如 "Claude.exe"）；取不到时为空串。
    pub process_name: String,
}

// ---------- 进程枚举 ----------

/// 全量进程映像名（小写）快照，用于目标「已安装/运行中」检测。
pub fn list_process_names() -> Result<Vec<String>> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(Win32Error::from)?;
        let mut names = Vec::new();
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let name = utf16_to_string(&entry.szExeFile);
                if !name.is_empty() {
                    names.push(name.to_lowercase());
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
        Ok(names)
    }
}

/// 目标进程是否在运行（按 process_names 任一匹配，大小写不敏感）。
pub fn is_running(process_names: &[String]) -> bool {
    if process_names.is_empty() {
        return false;
    }
    let running = list_process_names().unwrap_or_default();
    process_names
        .iter()
        .any(|p| running.iter().any(|r| r == &p.to_lowercase()))
}

// ---------- 窗口枚举与激活 ----------

struct EnumCtx {
    windows: Vec<WindowInfo>,
}

/// 枚举全部可见、有标题的顶层窗口。
pub fn list_windows() -> Vec<WindowInfo> {
    unsafe {
        let mut ctx = EnumCtx { windows: Vec::new() };
        let _ = EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut ctx as *mut EnumCtx as isize),
        );
        ctx.windows
    }
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut EnumCtx);
    if IsWindowVisible(hwnd).as_bool() {
        let title = window_title(hwnd);
        if !title.is_empty() {
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            ctx.windows.push(WindowInfo {
                hwnd: hwnd.0 as isize,
                pid,
                title,
                process_name: process_name_of(pid).unwrap_or_default(),
            });
        }
    }
    BOOL(1)
}

fn window_title(hwnd: HWND) -> String {
    unsafe {
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..len as usize])
    }
}

fn process_name_of(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        ok.ok()?;
        let full = String::from_utf16_lossy(&buf[..size as usize]);
        Some(
            Path::new(&full)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
        )
    }
}

/// 按进程名 + 窗口标题关键字找到目标窗口（进程名优先精确，标题做包含匹配）。
pub fn find_window(process_names: &[String], title_keywords: &[String]) -> Option<WindowInfo> {
    let windows = list_windows();
    let procs: Vec<String> = process_names.iter().map(|p| p.to_lowercase()).collect();

    if !procs.is_empty() {
        if let Some(w) = windows
            .iter()
            .find(|w| procs.contains(&w.process_name.to_lowercase()))
        {
            return Some(w.clone());
        }
    }
    if !title_keywords.is_empty() {
        return windows
            .into_iter()
            .find(|w| title_keywords.iter().any(|k| !k.is_empty() && w.title.contains(k)));
    }
    None
}

/// 激活窗口：还原最小化 + AttachThreadInput 绕过焦点保护 + 置前。
pub fn activate_window(hwnd: isize) -> Result<()> {
    unsafe {
        let hwnd = HWND(hwnd as *mut std::ffi::c_void);
        let _ = ShowWindow(hwnd, SW_RESTORE);

        let foreground = GetForegroundWindow();
        let current_thread = GetCurrentThreadId();
        let foreground_thread = GetWindowThreadProcessId(foreground, None);
        let target_thread = GetWindowThreadProcessId(hwnd, None);

        let mut attached: Vec<u32> = Vec::new();
        for other in [foreground_thread, target_thread] {
            if other != 0 && other != current_thread {
                if AttachThreadInput(current_thread, other, true).as_bool() {
                    attached.push(other);
                }
            }
        }
        let _ = BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd).ok().map_err(Win32Error::from)?;
        for other in attached {
            let _ = AttachThreadInput(current_thread, other, false);
        }
        Ok(())
    }
}

// ---------- 剪贴板 ----------

/// 带重试地打开剪贴板（其他应用可能短暂持有）。
fn with_clipboard<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    unsafe {
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(None).is_ok() {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if !opened {
            return Err(Win32Error::ClipboardBusy);
        }
        let result = f();
        let _ = CloseClipboard();
        result
    }
}

/// 写入文本到剪贴板（CF_UNICODETEXT）。
pub fn set_clipboard_text(text: &str) -> Result<()> {
    with_clipboard(|| unsafe {
        EmptyClipboard().map_err(Win32Error::from)?;
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = wide.len() * 2;
        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(Win32Error::from)?;
        let ptr = GlobalLock(handle);
        if ptr.is_null() {
            return Err(Win32Error::Api("GlobalLock 返回空指针".into()));
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr as *mut u8, bytes);
        let _ = GlobalUnlock(handle);
        // 成功后句柄归系统所有，不再释放。
        SetClipboardData(CF_UNICODETEXT.0 as u32, windows::Win32::Foundation::HANDLE(handle.0))
            .map_err(Win32Error::from)?;
        Ok(())
    })
}

/// 写入文件列表到剪贴板（CF_HDROP，DROPFILES + 双零结尾宽字符列表）。
pub fn set_clipboard_files(paths: &[PathBuf]) -> Result<()> {
    if paths.is_empty() {
        return Err(Win32Error::InvalidInput("文件列表为空".into()));
    }
    with_clipboard(|| unsafe {
        EmptyClipboard().map_err(Win32Error::from)?;

        let mut wide: Vec<u16> = Vec::new();
        for p in paths {
            wide.extend(p.to_string_lossy().encode_utf16());
            wide.push(0);
        }
        wide.push(0); // 列表结束的双零

        let header = std::mem::size_of::<DROPFILES>();
        let bytes = header + wide.len() * 2;
        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(Win32Error::from)?;
        let ptr = GlobalLock(handle);
        if ptr.is_null() {
            return Err(Win32Error::Api("GlobalLock 返回空指针".into()));
        }

        let drop = DROPFILES {
            pFiles: header as u32,
            pt: POINT { x: 0, y: 0 },
            fNC: BOOL(0),
            fWide: BOOL(1),
        };
        std::ptr::write(ptr as *mut DROPFILES, drop);
        std::ptr::copy_nonoverlapping(
            wide.as_ptr(),
            (ptr as *mut u8).add(header) as *mut u16,
            wide.len(),
        );
        let _ = GlobalUnlock(handle);
        SetClipboardData(CF_HDROP.0 as u32, windows::Win32::Foundation::HANDLE(handle.0))
            .map_err(Win32Error::from)?;
        Ok(())
    })
}

/// 读取剪贴板文本（测试与兜底用）。
pub fn get_clipboard_text() -> Result<Option<String>> {
    with_clipboard(|| unsafe {
        let handle = GetClipboardData(CF_UNICODETEXT.0 as u32);
        let Ok(handle) = handle else {
            return Ok(None);
        };
        if handle.0.is_null() {
            return Ok(None);
        }
        let ptr = GlobalLock(windows::Win32::Foundation::HGLOBAL(handle.0));
        if ptr.is_null() {
            return Ok(None);
        }
        let mut len = 0usize;
        let slice = ptr as *const u16;
        while *slice.add(len) != 0 {
            len += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(slice, len));
        let _ = GlobalUnlock(windows::Win32::Foundation::HGLOBAL(handle.0));
        Ok(Some(text))
    })
}

// ---------- 键盘模拟 ----------

/// 模拟 Ctrl+V（SendInput，应用级，无需权限）。
pub fn send_ctrl_v() -> Result<()> {
    unsafe {
        let mk = |vk: u16, up: bool| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk),
                    wScan: 0,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let inputs = [
            mk(VK_CONTROL.0, false),
            mk(VK_V.0, false),
            mk(VK_V.0, true),
            mk(VK_CONTROL.0, true),
        ];
        let sent = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        if sent as usize != inputs.len() {
            return Err(Win32Error::Api(format!(
                "SendInput 只发送了 {sent}/{} 个事件",
                inputs.len()
            )));
        }
        Ok(())
    }
}

/// 启动目标 exe（目标未运行时）。
pub fn launch_exe(path: &Path) -> Result<()> {
    std::process::Command::new(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| Win32Error::Api(format!("启动 {} 失败: {e}", path.display())))
}

// ---------- 工具 ----------

fn utf16_to_string(slice: &[u16]) -> String {
    let len = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
    String::from_utf16_lossy(&slice[..len])
}

// WinResult/BOOL/CHAR 等类型占位避免未使用告警（部分 API 未来波次使用）。
#[allow(dead_code)]
type _Unused = (WinResult<()>, WPARAM);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_list_is_nonempty() {
        let names = list_process_names().unwrap();
        assert!(names.len() > 10, "进程快照不应为空");
        assert!(names.iter().all(|n| n == &n.to_lowercase()));
    }

    #[test]
    fn clipboard_text_round_trip() {
        let sample = format!("WeChatBridge 剪贴板测试 {}", uuid::Uuid::new_v4());
        set_clipboard_text(&sample).unwrap();
        let read = get_clipboard_text().unwrap();
        assert_eq!(read.as_deref(), Some(sample.as_str()));
    }

    #[test]
    fn clipboard_files_rejects_empty() {
        assert!(matches!(
            set_clipboard_files(&[]),
            Err(Win32Error::InvalidInput(_))
        ));
    }

    #[test]
    fn window_enumeration_runs() {
        // 测试环境可能没有可见窗口；只验证 API 不崩溃。
        let _ = list_windows();
    }
}
