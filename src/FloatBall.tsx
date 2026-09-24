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

/** 菜单面板宽度；菜单展开时窗口高。 */
const MENU_PANEL_W = 184;
const MENU_GAP = 8;
const MENU_H = 330;
const MIN_SIZE = 48;
const MAX_SIZE = 120;

/** 目标图标：取目标名首字符。 */
const targetGlyph = (name: string) => name.slice(0, 1);

/**
 * 拖拽悬浮球：始终置顶，接收从微信/资源管理器拖入的 ZIP。
 * 玩法：拖拽移动（松手贴边）· 滚轮缩放 · 双击开主窗口 · 右键目标网格菜单
 *       · 待处理批次呼吸光环 + 角标 · 空闲半透明 · 成功波纹 / 失败抖动。
 */
export default function FloatBall() {
  const [status, setStatus] = useState<Status>("idle");
  const [msg, setMsg] = useState("");
  const [ballColor, setBallColor] = useState("#07c160");
  const [ballSize, setBallSize] = useState(72);
  const [menuOpen, setMenuOpen] = useState(false);
  const [targets, setTargets] = useState<TargetStatus[]>([]);
  const [faded, setFaded] = useState(false);
  const [pendingCount, setPendingCount] = useState(0);
  const [sizeTip, setSizeTip] = useState("");
  const suppressClick = useRef(false);
  const idleTimer = useRef<number | undefined>(undefined);
  const statusTimer = useRef<number | undefined>(undefined);
  const wheelTimer = useRef<number | undefined>(undefined);
  const prevPos = useRef<{ x: number; y: number } | null>(null);

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

  // 窗口尺寸跟随球大小（菜单关闭时）。
  useEffect(() => {
    if (!menuOpen) {
      win.setSize(new LogicalSize(ballSize, ballSize)).catch(() => {});
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ballSize, menuOpen]);

  // 待处理（已接收未转发）批次角标：轮询刷新。
  const refreshPending = async () => {
    try {
      const batches = await api.listBatches();
      setPendingCount(batches.filter((b) => b.state === "received").length);
    } catch {
      /* 忽略 */
    }
  };
  useEffect(() => {
    refreshPending();
    const t = window.setInterval(refreshPending, 8000);
    return () => window.clearInterval(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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
          refreshPending();
          flash("ok", `已接收 ${m.displayName.slice(0, 12)}`, 1400);
          setTimeout(() => showMain(), 800);
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
    if (e.button !== 0 || menuOpen) return;
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
      const snapX = x + ballSize / 2 < mw / 2 ? margin : mw - ballSize - margin;
      const snapY = Math.min(Math.max(y, margin), mh - ballSize - margin);
      await win.setPosition(new LogicalPosition(snapX, snapY));
    } catch {
      /* 忽略 */
    }
  };

  // 滚轮直接缩放（防抖保存）。
  const onWheel = (e: React.WheelEvent) => {
    resetIdle();
    const delta = e.deltaY > 0 ? -8 : 8;
    const next = Math.min(MAX_SIZE, Math.max(MIN_SIZE, ballSize + delta));
    if (next === ballSize) return;
    setBallSize(next);
    setSizeTip(`${next}px`);
    window.clearTimeout(wheelTimer.current);
    wheelTimer.current = window.setTimeout(async () => {
      setSizeTip("");
      try {
        const s = await api.getSettings();
        await api.saveSettings({ ...s, ballSize: next });
      } catch {
        /* 忽略 */
      }
    }, 600);
  };

  // 右键菜单：开窗（窗口扩大容纳菜单），关闭时还原位置与尺寸。
  const openMenu = async (e: React.MouseEvent) => {
    e.preventDefault();
    resetIdle();
    if (menuOpen) {
      closeMenu();
      return;
    }
    try {
      setTargets(await api.listTargets());
    } catch {
      setTargets([]);
    }
    // 记录原位置，若菜单超出屏幕右/下缘则挪动窗口。
    try {
      const menuW = ballSize + MENU_GAP + MENU_PANEL_W + 8;
      const pos = await win.outerPosition();
      const monitor = await currentMonitor();
      const scale = monitor?.scaleFactor ?? 1;
      const x = pos.x / scale;
      const y = pos.y / scale;
      prevPos.current = { x, y };
      const mw = monitor ? monitor.size.width / scale : x + menuW;
      const mh = monitor ? monitor.size.height / scale : y + MENU_H;
      const nx = Math.min(x, Math.max(0, mw - menuW - 8));
      const ny = Math.min(y, Math.max(0, mh - MENU_H - 8));
      await win.setSize(new LogicalSize(menuW, MENU_H));
      if (nx !== x || ny !== y) {
        await win.setPosition(new LogicalPosition(nx, ny));
      }
    } catch {
      /* 忽略 */
    }
    setMenuOpen(true);
  };

  const closeMenu = async () => {
    setMenuOpen(false);
    try {
      await win.setSize(new LogicalSize(ballSize, ballSize));
      if (prevPos.current) {
        await win.setPosition(
          new LogicalPosition(prevPos.current.x, prevPos.current.y),
        );
        prevPos.current = null;
      }
    } catch {
      /* 忽略 */
    }
  };

  // 菜单动作：快捷转发最近批次 / 打开主窗口 / 隐藏 / 尺寸预设。
  const runAction = async (key: string, kind: "target" | "action") => {
    await closeMenu();
    if (kind === "action") {
      if (key === "open") await showMain();
      if (key === "hide") {
        // 走 Rust 命令隐藏（前端 win.hide() 受窗口权限链路影响，可能静默失败）。
        try {
          await api.hideFloatBall();
        } catch {
          await win.hide().catch(() => {});
        }
      }
      return;
    }
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
      const r = await api.forwardBatch(latest.id, key, scenes.defaultSceneId);
      refreshPending();
      flash("ok", r.message.slice(0, 16), 2000);
    } catch (err) {
      flash("error", String(err).slice(0, 24), 2400);
    }
  };

  const menuTargets = targets.filter(
    (t) => t.enabled && t.id !== "custom" && t.id !== "clipboard",
  );

  const innerSize = Math.max(36, ballSize - 16);
  const breathing = pendingCount > 0 && status === "idle";

  return (
    <div
      className={[
        "float-ball",
        menuOpen ? "menu-mode" : "",
        status,
        faded && status === "idle" && !menuOpen ? "faded" : "",
        breathing ? "breathe" : "",
      ].join(" ")}
      style={
        {
          "--ball-color": ballColor,
          "--ball-size": `${ballSize}px`,
        } as React.CSSProperties
      }
      onMouseDown={startDrag}
      onMouseEnter={resetIdle}
      onMouseMove={resetIdle}
      onWheel={onWheel}
      onClick={() => {
        if (!suppressClick.current && !menuOpen) void showMain();
      }}
      onContextMenu={openMenu}
      title="拖入 ZIP 导入 · 拖拽移动 · 滚轮缩放 · 双击开主窗口 · 右键目标菜单"
    >
      <div className="ball-inner" style={{ width: innerSize, height: innerSize }}>
        {status === "loading" && <span className="spinner" />}
        {status === "ok" && <span className="glyph pop">✓</span>}
        {status === "error" && <span className="glyph">!</span>}
        {(status === "idle" || status === "over") && (
          <span className="glyph">微</span>
        )}
      </div>

      {/* 待处理批次角标 */}
      {pendingCount > 0 && status === "idle" && !menuOpen && (
        <span className="ball-badge">{pendingCount > 9 ? "9+" : pendingCount}</span>
      )}

      {/* 成功波纹 */}
      {status === "ok" && (
        <>
          <span className="ball-ripple" />
          <span className="ball-ripple delay" />
        </>
      )}

      {/* 拖入脉冲光环 */}
      {status === "over" && (
        <>
          <span className="pulse-ring" />
          <span className="pulse-ring delay" />
        </>
      )}

      {/* 状态 / 尺寸提示 */}
      {(status === "loading" || status === "ok" || status === "error") && msg && (
        <div className="ball-tip">{msg}</div>
      )}
      {sizeTip && status === "idle" && !menuOpen && (
        <div className="ball-tip size">{sizeTip}</div>
      )}

      {menuOpen && (
        <>
          <div className="ball-menu-mask" onClick={closeMenu} />
          <div className="ball-menu">
            <div className="ball-menu-title">转发最近批次到</div>
            {menuTargets.length === 0 && (
              <div className="ball-menu-empty">暂无可用目标</div>
            )}
            <div className="ball-menu-grid">
              {menuTargets.map((t) => (
                <button
                  key={t.id}
                  className="target-cell"
                  onClick={() => runAction(t.id, "target")}
                  title={t.displayName}
                >
                  <span
                    className={`target-icon ${t.running || t.id === "obsidian" ? "on" : ""}`}
                  >
                    {targetGlyph(t.displayName)}
                  </span>
                  <span className="target-name">{t.displayName}</span>
                </button>
              ))}
            </div>
            <div className="ball-menu-sep" />
            <div className="ball-menu-actions">
              <button onClick={() => runAction("open", "action")}>主窗口</button>
              <button onClick={() => runAction("hide", "action")}>隐藏</button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
