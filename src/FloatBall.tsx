import { useEffect, useRef, useState } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listen } from "@tauri-apps/api/event";
import { importZip, getSettings } from "./api";
import "./FloatBall.css";

/**
 * 拖拽悬浮球：始终置顶，接收从微信/资源管理器拖入的 ZIP。
 * 支持：拖拽移动、右键隐藏、自定义颜色/尺寸、双击打开主窗口。
 */
export default function FloatBall() {
  const [status, setStatus] = useState<"idle" | "loading" | "ok" | "error">("idle");
  const [msg, setMsg] = useState("");
  const [ballColor, setBallColor] = useState("#6c5ce7");
  const [ballSize, setBallSize] = useState(72);
  const [menuOpen, setMenuOpen] = useState(false);
  const [dragging, setDragging] = useState(false);
  const dragMoved = useRef(false);

  // 加载设置中的颜色和尺寸。
  useEffect(() => {
    getSettings()
      .then((s) => {
        setBallColor(s.ballColor || "#6c5ce7");
        setBallSize(s.ballSize || 72);
      })
      .catch(() => {});
    // 监听设置变更事件，实时更新悬浮球。
    const unlisten = listen("settings-updated", (e: any) => {
      if (e.payload?.ballColor) setBallColor(e.payload.ballColor);
      if (e.payload?.ballSize) setBallSize(e.payload.ballSize);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const showMain = async () => {
    const main = await WebviewWindow.getByLabel("main");
    await main?.show();
    await main?.unminimize();
    await main?.setFocus();
  };

  const hideBall = async () => {
    await getCurrentWindow().hide();
  };

  // 拖拽移动窗口。
  const startDrag = async (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    dragMoved.current = false;
    setDragging(true);
    try {
      await getCurrentWindow().startDragging();
    } catch {
      /* ignore */
    }
    setDragging(false);
  };

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    const files = Array.from(e.dataTransfer.files);
    const zip = files.find((f) => f.name.toLowerCase().endsWith(".zip"));
    if (!zip) {
      setStatus("error");
      setMsg("请拖入 ZIP 文件");
      setTimeout(() => setStatus("idle"), 1500);
      return;
    }
    setStatus("loading");
    setMsg("导入中…");
    try {
      const path = (zip as any).path || zip.name;
      await importZip(path);
      setStatus("ok");
      setMsg("已导入");
      setTimeout(() => {
        setStatus("idle");
        showMain();
      }, 800);
    } catch (err) {
      setStatus("error");
      setMsg(String(err).slice(0, 30));
      setTimeout(() => setStatus("idle"), 2000);
    }
  };

  const handleClick = () => {
    if (dragMoved.current) return; // 拖拽后不触发点击
    showMain();
  };

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    setMenuOpen(true);
  };

  // 动态调整窗口尺寸以匹配球大小。
  useEffect(() => {
    const w = getCurrentWindow();
    w.setSize(new LogicalSize(ballSize, ballSize)).catch(() => {});
  }, [ballSize]);

  const innerSize = Math.max(36, ballSize - 16);

  return (
    <div
      className={`float-ball ${status} ${dragging ? "dragging" : ""}`}
      style={
        {
          "--ball-color": ballColor,
          "--ball-size": `${ballSize}px`,
        } as React.CSSProperties
      }
      onMouseDown={startDrag}
      onDragOver={(e) => e.preventDefault()}
      onDrop={handleDrop}
      onClick={handleClick}
      onContextMenu={handleContextMenu}
      onDoubleClick={showMain}
      title="拖入 ZIP · 双击打开主窗口 · 右键隐藏"
    >
      <div className="ball-inner" style={{ width: innerSize, height: innerSize }}>
        {status === "loading" && <span className="spinner" />}
        {status === "ok" && <span className="glyph">✓</span>}
        {status === "error" && <span className="glyph">!</span>}
        {status === "idle" && <span className="glyph">微</span>}
      </div>
      {status !== "idle" && <div className="ball-tip">{msg}</div>}

      {menuOpen && (
        <div className="ball-menu" onClick={(e) => e.stopPropagation()}>
          <button
            onClick={() => {
              setMenuOpen(false);
              showMain();
            }}
          >
            打开主窗口
          </button>
          <button
            onClick={() => {
              setMenuOpen(false);
              hideBall();
            }}
          >
            隐藏悬浮球
          </button>
        </div>
      )}
    </div>
  );
}
