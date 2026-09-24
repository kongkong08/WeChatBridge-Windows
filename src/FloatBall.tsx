import { useEffect, useRef, useState } from "react";
import {
  getCurrentWindow,
  LogicalSize,
  LogicalPosition,
  currentMonitor,
} from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import * as api from "./api";
import type { TargetStatus } from "./types";
import "./FloatBall.css";

type Status = "idle" | "over" | "loading" | "ok" | "error";

/** 悬浮球右键菜单项。 */
interface MenuAction {
  key: string;
  label: string;
  kind: "target" | "action";
}

/**
 * 拖拽悬浮球：始终置顶，接收从微信/资源管理器拖入的 ZIP。
 * 玩法：拖拽移动（松手贴边吸附）· 双击开主窗口 · 右键快捷转发最近批次 · 空闲半透明。
 */
export default function FloatBall() {
  const [status, setStatus] = useState<Status>("idle");
  const [msg, setMsg] = useState("");
  const [ballColor, setBallColor] = useState("#07c160");
  const [ballSize, setBallSize] = useState(72);
  const [menuOpen, setMenuOpen] = useState(false);
  const [targets, setTargets] = useState<TargetStatus[]>([]);
  const [faded, setFaded] = useState(false);
  const suppressClick = useRef(false);
  const idleTimer = useRef<number | undefined>(undefined);
  const statusTimer = useRef<number | undefined>(undefined);

  const win = getCurrentWindow();

  // 初始加载设置 + 监听设置变更。
  useEffect(() => {
    api
      .getSettings()
      .then((s) => {
        setBallColor(s.ballColor || "#07c160");
        setBallSize(s.ballSize || 72);
      })
      .catch(() => {});
    const unlisten = listen<{ ballColor?: string; ballSize?: number }>(
      "settings-updated",
      (e) => {
        if (e.payload.ballColor) setBallColor(e.payload.ballColor);
        if (e.payload.ballSize) setBallSize(e.payload.ballSize);
      },
    );
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // 窗口尺寸跟随球大小。
  useEffect(() => {
    win.setSize(new LogicalSize(ballSize, ballSize)).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ballSize]);

  // 空闲 3 秒半透明，鼠标进入恢复。
  const resetIdle = () => {
    setFaded(false);
    window.clearTimeout(idleTimer.current);
    idleTimer.current = window.setTimeout(() => setFaded(true), 3000);
  };
  useEffect(() => {
    resetIdle();
    return () => window.clearTimeout(idleTimer.current);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const flash = (s: Status, text: string, ms = 1800) => {
    setStatus(s);
    setMsg(text);
    window.clearTimeout(statusTimer.current);
    statusTimer.current = window.setTimeout(() => setStatus("idle"), ms);
  };

  const showMain = async () => {
    const main = await WebviewWindow.getByLabel("main");
    await main?.show();
    await main?.unminimize();
    await main?.setFocus();
  };

  // Tauri 拖放事件（带真实文件路径）。
  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent(async (event) => {
      if (event.payload.type === "over") {
        setStatus("over");
      } else if (event.payload.type === "leave") {
        setStatus("idle");
      } else if (event.payload.type === "drop") {
        const zip = event.payload.paths.find((p) =>
          p.toLowerCase().endsWith(".zip"),
        );
        if (!zip) {
          flash("error", "请拖入 ZIP 文件");
          return;
        }
        setStatus("loading");
        setMsg("导入中…");
        try {
          const m = await api.importZip(zip);
          flash("ok", `已接收 ${m.displayName.slice(0, 12)}`, 1200);
          setTimeout(() => showMain(), 700);
        } catch (err) {
          flash("error", String(err).slice(0, 24), 2400);
        }
      }
    });
    return () => {
      unlisten.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 拖拽移动 + 松手贴边吸附。
  const startDrag = async (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    resetIdle();
    try {
      await win.startDragging();
      suppressClick.current = true;
      window.setTimeout(() => (suppressClick.current = false), 150);
      await snapToEdge();
    } catch {
      /* 权限或平台不支持时忽略 */
    }
  };

  const snapToEdge = async () => {
    try {
      const pos = await win.outerPosition();
      const monitor = await currentMonitor();
      if (!monitor) return;
      const scale = monitor.scaleFactor;
      const mw = monitor.size.width / scale;
      const mh = monitor.size.height / scale;
      const x = pos.x / scale;
      const y = pos.y / scale;
      const margin = 12;
      const snapX =
        x + ballSize / 2 < mw / 2 ? margin : mw - ballSize - margin;
      const snapY = Math.min(Math.max(y, margin), mh - ballSize - margin);
      await win.setPosition(new LogicalPosition(snapX, snapY));
    } catch {
      /* 忽略 */
    }
  };

  const openMenu = async (e: React.MouseEvent) => {
    e.preventDefault();
    setMenuOpen((v) => !v);
    try {
      setTargets(await api.listTargets());
    } catch {
      setTargets([]);
    }
  };

  // 右键菜单动作：快捷转发最近批次 / 打开主窗口 / 隐藏。
  const runAction = async (a: MenuAction) => {
    setMenuOpen(false);
    if (a.kind === "action") {
      if (a.key === "open") await showMain();
      if (a.key === "hide") await win.hide();
      if (a.key.startsWith("size:")) {
        const size = Number(a.key.slice(5));
        const s = await api.getSettings();
        await api.saveSettings({ ...s, ballSize: size });
      }
      return;
    }
    // 快捷转发：取最近一个批次 → 用默认场景转发到所选目标。
    setStatus("loading");
    setMsg("转发中…");
    try {
      const batches = await api.listBatches();
      const latest = batches[0];
      if (!latest) {
        flash("error", "暂无批次");
        return;
      }
      const scenes = await api.listScenes();
      const r = await api.forwardBatch(
        latest.id,
        a.key,
        scenes.defaultSceneId,
      );
      flash("ok", r.message.slice(0, 16), 2000);
    } catch (err) {
      flash("error", String(err).slice(0, 24), 2400);
    }
  };

  const menuTargets = targets.filter(
    (t) => t.enabled && t.id !== "custom" && t.id !== "clipboard",
  );

  const innerSize = Math.max(36, ballSize - 16);

  return (
    <div
      className={[
        "float-ball",
        status,
        faded && status === "idle" && !menuOpen ? "faded" : "",
      ].join(" ")}
      style={{ "--ball-color": ballColor } as React.CSSProperties}
      onMouseDown={startDrag}
      onMouseEnter={resetIdle}
      onMouseMove={resetIdle}
      onClick={() => {
        if (!suppressClick.current) void showMain();
      }}
      onContextMenu={openMenu}
      title="拖入 ZIP 导入 · 拖拽移动 · 双击开主窗口 · 右键快捷转发"
    >
      <div className="ball-inner" style={{ width: innerSize, height: innerSize }}>
        {status === "loading" && <span className="spinner" />}
        {status === "ok" && <span className="glyph">✓</span>}
        {status === "error" && <span className="glyph">!</span>}
        {(status === "idle" || status === "over") && (
          <span className="glyph">微</span>
        )}
      </div>
      {status !== "idle" && status !== "over" && msg && (
        <div className="ball-tip">{msg}</div>
      )}

      {menuOpen && (
        <>
          <div className="ball-menu-mask" onClick={() => setMenuOpen(false)} />
          <div className="ball-menu">
            <div className="ball-menu-title">快捷转发（最近批次）</div>
            {menuTargets.length === 0 && (
              <div className="ball-menu-empty">暂无可用目标</div>
            )}
            {menuTargets.map((t) => (
              <button
                key={t.id}
                onClick={() => runAction({ key: t.id, label: t.displayName, kind: "target" })}
              >
                <span className={`dot ${t.running || t.id === "obsidian" ? "on" : ""}`} />
                {t.displayName}
              </button>
            ))}
            <div className="ball-menu-sep" />
            <div className="ball-menu-title">大小</div>
            <div className="ball-menu-sizes">
              {[56, 72, 96].map((s) => (
                <button
                  key={s}
                  className={s === ballSize ? "size-btn active" : "size-btn"}
                  onClick={() =>
                    runAction({ key: `size:${s}`, label: "", kind: "action" })
                  }
                >
                  {s === 56 ? "小" : s === 72 ? "中" : "大"}
                </button>
              ))}
            </div>
            <div className="ball-menu-sep" />
            <button onClick={() => runAction({ key: "open", label: "", kind: "action" })}>
              打开主窗口
            </button>
            <button onClick={() => runAction({ key: "hide", label: "", kind: "action" })}>
              隐藏悬浮球
            </button>
          </div>
        </>
      )}
    </div>
  );
}
