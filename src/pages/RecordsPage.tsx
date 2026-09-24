import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import * as api from "../api";
import type {
  BatchManifest,
  Preview,
  TargetStatus,
  WeChatScene,
} from "../types";

const STATE_LABEL: Record<BatchManifest["state"], string> = {
  received: "已接收",
  delivered: "已送达",
  copied: "已复制",
  failed: "失败",
  expired: "已过期",
};

function fmtTime(iso: string): string {
  return iso.replace("T", " ").slice(0, 16);
}

export default function RecordsPage() {
  const [batches, setBatches] = useState<BatchManifest[]>([]);
  const [targets, setTargets] = useState<TargetStatus[]>([]);
  const [scenes, setScenes] = useState<WeChatScene[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [checkedIds, setCheckedIds] = useState<Set<string>>(new Set());
  const [preview, setPreview] = useState<Preview | null>(null);
  const [targetId, setTargetId] = useState<string>("claude");
  const [sceneId, setSceneId] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [toast, setToast] = useState<string>("");
  const [dragOver, setDragOver] = useState(false);
  const [showGuide, setShowGuide] = useState(true);
  const toastTimer = useRef<number | undefined>(undefined);

  const showToast = useCallback((msg: string) => {
    setToast(msg);
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(""), 4000);
  }, []);

  const refresh = useCallback(async () => {
    const [b, t, s] = await Promise.all([
      api.listBatches(),
      api.listTargets(),
      api.listScenes(),
    ]);
    setBatches(b);
    setTargets(t);
    setScenes(s.scenes.filter((x) => x.enabled));
  }, []);

  useEffect(() => {
    refresh().catch((e) => showToast(String(e)));
  }, [refresh, showToast]);

  // Tauri 拖放事件（真实文件路径）
  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent(async (event) => {
      if (event.payload.type === "over") {
        setDragOver(true);
      } else if (event.payload.type === "leave") {
        setDragOver(false);
      } else if (event.payload.type === "drop") {
        setDragOver(false);
        const zip = event.payload.paths.find((p) =>
          p.toLowerCase().endsWith(".zip"),
        );
        if (!zip) {
          showToast("请拖入微信导出的 ZIP 文件");
          return;
        }
        await doImport(zip);
      }
    });
    return () => {
      unlisten.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function doImport(path: string) {
    setBusy(true);
    try {
      const m = await api.importZip(path);
      showToast(`已接收：${m.displayName}`);
      await refresh();
      setSelectedId(m.id);
    } catch (e) {
      showToast(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function pickZip() {
    const path = await api.pickFile([["微信导出 ZIP", ["zip"]]]);
    if (path) await doImport(path);
  }

  async function select(id: string) {
    setSelectedId(id);
    setPreview(null);
    try {
      setPreview(await api.previewBatch(id));
    } catch (e) {
      showToast(String(e));
    }
  }

  async function doForward() {
    if (!selectedId) return;
    setBusy(true);
    try {
      const r = await api.forwardBatch(
        selectedId,
        targetId,
        sceneId || null,
      );
      showToast(r.message);
      await refresh();
      if (targetId === "obsidian") setPreview(await api.previewBatch(selectedId));
    } catch (e) {
      showToast(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function doCopy() {
    if (!selectedId) return;
    setBusy(true);
    try {
      const what = await api.copyManualPayload(
        selectedId,
        targetId,
        sceneId || null,
      );
      showToast(`已复制到剪贴板：${what}，请到目标窗口按 Ctrl+V`);
      await refresh();
    } catch (e) {
      showToast(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function doDelete(id: string) {
    await api.deleteBatch(id);
    if (selectedId === id) {
      setSelectedId(null);
      setPreview(null);
    }
    await refresh();
  }

  function toggleCheck(id: string) {
    setCheckedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  async function doBatchForward() {
    if (checkedIds.size === 0) return;
    setBusy(true);
    let ok = 0;
    let fail = 0;
    for (const id of checkedIds) {
      try {
        await api.forwardBatch(id, targetId, sceneId || null);
        ok++;
      } catch {
        fail++;
      }
    }
    showToast(
      `批量转发完成：成功 ${ok} 个${fail ? `，失败 ${fail} 个` : ""}`,
    );
    setCheckedIds(new Set());
    await refresh();
    setBusy(false);
  }

  async function doBatchDelete() {
    if (checkedIds.size === 0) return;
    setBusy(true);
    for (const id of checkedIds) {
      await api.deleteBatch(id).catch(() => {});
    }
    if (selectedId && checkedIds.has(selectedId)) {
      setSelectedId(null);
      setPreview(null);
    }
    showToast(`已删除 ${checkedIds.size} 个批次`);
    setCheckedIds(new Set());
    await refresh();
    setBusy(false);
  }

  const selected = batches.find((b) => b.id === selectedId) ?? null;
  const enabledTargets = targets.filter((t) => t.enabled && t.id !== "custom");

  return (
    <div className="records-page">
      <section
        className={dragOver ? "dropzone over" : "dropzone"}
        onClick={pickZip}
      >
        <p className="dropzone-title">
          {busy ? "处理中…" : "拖入微信「合并转发」导出的 ZIP"}
        </p>
        <p className="dropzone-hint">或点击选择文件 · 也可拖到桌面悬浮球</p>
      </section>

      {showGuide && (
        <section className="guide-card">
          <div className="guide-head">
            <span className="guide-title">💡 如何获取微信聊天 ZIP？</span>
            <button
              className="guide-close"
              onClick={() => setShowGuide(false)}
              aria-label="关闭"
            >
              ×
            </button>
          </div>
          <ol className="guide-steps">
            <li>
              <b>转发给元宝AI</b>：在微信里多选聊天记录 → 合并转发 → 发给好友「元宝AI」，它会自动生成 ZIP 压缩包。
            </li>
            <li>
              <b>保存 ZIP</b>：在元宝AI聊天窗口里，把生成的 ZIP 文件另存到本地（或直接拖出）。
            </li>
            <li>
              <b>拖入悬浮球</b>：把 ZIP 拖到屏幕上的绿色「微」字悬浮球，或拖到上方区域，即可导入并转发到任意 AI。
            </li>
          </ol>
        </section>
      )}

      <div className="records-body">
        <section className="batch-list">
          <div className="batch-list-head">
            <h2>批次记录</h2>
            {batches.length > 0 && (
              <button
                className="link-btn"
                onClick={() =>
                  setCheckedIds(
                    checkedIds.size === batches.length
                      ? new Set()
                      : new Set(batches.map((b) => b.id)),
                  )
                }
              >
                {checkedIds.size === batches.length ? "取消全选" : "全选"}
              </button>
            )}
          </div>
          {checkedIds.size > 0 && (
            <div className="batch-toolbar">
              <span className="pill">已选 {checkedIds.size}</span>
              <button
                className="primary"
                disabled={busy}
                onClick={doBatchForward}
              >
                批量转发
              </button>
              <button disabled={busy} onClick={doBatchDelete}>
                批量删除
              </button>
            </div>
          )}
          {batches.length === 0 && <p className="empty">还没有归档记录</p>}
          {batches.map((b) => (
            <div
              key={b.id}
              className={
                b.id === selectedId ? "batch-item selected" : "batch-item"
              }
              onClick={() => select(b.id)}
            >
              <div className="batch-row">
                <input
                  type="checkbox"
                  className="batch-check"
                  checked={checkedIds.has(b.id)}
                  onClick={(e) => e.stopPropagation()}
                  onChange={() => toggleCheck(b.id)}
                />
                <span className="batch-name">{b.displayName}</span>
                <span className={`state state-${b.state}`}>
                  {STATE_LABEL[b.state]}
                </span>
              </div>
              <div className="batch-row sub">
                <span>{fmtTime(b.createdAt)}</span>
                {b.chatName && <span>{b.chatName}</span>}
                {b.sceneName && <span>场景：{b.sceneName}</span>}
                <button
                  className="link-btn danger"
                  onClick={(e) => {
                    e.stopPropagation();
                    doDelete(b.id);
                  }}
                >
                  删除
                </button>
              </div>
              {b.error && <div className="batch-error">{b.error}</div>}
            </div>
          ))}
        </section>

        <section className="detail">
          {!selected && <p className="empty">选择左侧批次查看预览与转发</p>}
          {selected && (
            <>
              <h2>{selected.displayName}</h2>
              {preview ? (
                <>
                  <p className="meta">
                    {preview.chatName ?? "未命名聊天"} ·{" "}
                    {preview.messageCount} 条消息 · {preview.attachments.length}{" "}
                    个附件
                  </p>
                  <div className="preview-list">
                    {preview.records.slice(0, 30).map((r, i) => (
                      <div className="msg" key={i}>
                        <div className="msg-head">
                          <b>{r.sender}</b>
                          <span>{fmtTime(r.date)}</span>
                        </div>
                        <div className="msg-text">{r.text}</div>
                      </div>
                    ))}
                    {preview.records.length > 30 && (
                      <p className="empty">
                        … 共 {preview.records.length} 条，仅预览前 30 条
                      </p>
                    )}
                  </div>
                </>
              ) : (
                <p className="empty">解析中或该批次无聊天记录…</p>
              )}

              <div className="forward-panel">
                <label>
                  目标
                  <select
                    value={targetId}
                    onChange={(e) => setTargetId(e.target.value)}
                  >
                    {enabledTargets.map((t) => (
                      <option key={t.id} value={t.id}>
                        {t.displayName}
                        {t.builtin && t.id !== "clipboard" && t.id !== "obsidian"
                          ? t.running
                            ? "（运行中）"
                            : "（未运行）"
                          : ""}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  场景
                  <select
                    value={sceneId}
                    onChange={(e) => setSceneId(e.target.value)}
                  >
                    <option value="">不使用场景</option>
                    {scenes.map((s) => (
                      <option key={s.id} value={s.id}>
                        {s.name}
                      </option>
                    ))}
                  </select>
                </label>
                <div className="forward-actions">
                  <button
                    className="primary"
                    disabled={busy}
                    onClick={doForward}
                  >
                    转发
                  </button>
                  <button disabled={busy} onClick={doCopy}>
                    复制到剪贴板
                  </button>
                  <button
                    disabled={busy}
                    onClick={() => api.openBatchFolder(selected.id)}
                  >
                    打开所在文件夹
                  </button>
                </div>
              </div>
            </>
          )}
        </section>
      </div>

      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}
