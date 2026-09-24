import { useState } from "react";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { importZip } from "./api";
import "./FloatBall.css";

/**
 * 拖拽悬浮球：始终置顶，接收从微信/资源管理器拖入的 ZIP。
 * 窗口 label 为 "float-ball"，由 Rust 端在启动时创建。
 */
export default function FloatBall() {
  const [hover, setHover] = useState(false);
  const [status, setStatus] = useState<"idle" | "loading" | "ok" | "error">("idle");
  const [msg, setMsg] = useState("");

  const showMain = async () => {
    const main = await WebviewWindow.getByLabel("main");
    await main?.show();
    await main?.unminimize();
    await main?.setFocus();
  };

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    setHover(false);
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
      // Tauri 拖放的 file 带有 path 属性（绝对路径）。
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

  return (
    <div
      className={`float-ball ${hover ? "hover" : ""} ${status}`}
      onDragOver={(e) => {
        e.preventDefault();
        setHover(true);
      }}
      onDragLeave={() => setHover(false)}
      onDrop={handleDrop}
      onClick={showMain}
      title="拖入微信导出的 ZIP，或点击打开主窗口"
    >
      <div className="ball-inner">
        {status === "loading" && <span className="spinner" />}
        {status === "ok" && <span className="glyph">✓</span>}
        {status === "error" && <span className="glyph">!</span>}
        {status === "idle" && <span className="glyph">微</span>}
      </div>
      {status !== "idle" && <div className="ball-tip">{msg}</div>}
    </div>
  );
}
