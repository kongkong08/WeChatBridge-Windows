import { useCallback, useEffect, useState } from "react";
import * as api from "../api";
import type {
  AppSettings,
  SceneSettings,
  SkillInfo,
  TargetStatus,
} from "../types";

export default function SettingsPage() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [scenes, setScenes] = useState<SceneSettings | null>(null);
  const [targets, setTargets] = useState<TargetStatus[]>([]);
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [toast, setToast] = useState("");

  const showToast = useCallback((msg: string) => {
    setToast(msg);
    window.setTimeout(() => setToast(""), 3500);
  }, []);

  const load = useCallback(async () => {
    const [s, sc, t, sk] = await Promise.all([
      api.getSettings(),
      api.listScenes(),
      api.listTargets(),
      api.listSkills(),
    ]);
    setSettings(s);
    setScenes(sc);
    setTargets(t);
    setSkills(sk);
  }, []);

  useEffect(() => {
    load().catch((e) => showToast(String(e)));
  }, [load, showToast]);

  if (!settings || !scenes) return <p className="empty">加载中…</p>;

  async function updateSettings(patch: Partial<AppSettings>) {
    const next = { ...settings!, ...patch };
    setSettings(next);
    await api.saveSettings(next);
    showToast("设置已保存");
  }

  async function updateScenes(next: SceneSettings) {
    setScenes(next);
    await api.saveScenes(next);
  }

  async function updateTargets(next: TargetStatus[]) {
    setTargets(next);
    await api.saveTargets(next);
    showToast("目标已保存");
  }

  async function installSkill() {
    const path = await api.pickFile([["SKILL 定义", ["md", "yaml", "json"]]]);
    if (!path) return;
    try {
      const s = await api.installSkill(path);
      showToast(`已安装技能：${s.name}`);
      setSkills(await api.listSkills());
    } catch (e) {
      showToast(String(e));
    }
  }

  return (
    <div className="settings-page">
      <section className="card">
        <h2>通用</h2>
        <label className="row">
          界面语言
          <select
            value={settings.language}
            onChange={(e) => updateSettings({ language: e.target.value })}
          >
            <option value="zh">中文</option>
            <option value="en">English</option>
          </select>
        </label>
        <label className="row">
          文件送达方式
          <select
            value={settings.pasteMode}
            onChange={(e) =>
              updateSettings({
                pasteMode: e.target.value as AppSettings["pasteMode"],
              })
            }
          >
            <option value="textWithPaths">提示词 + 路径文本（推荐，兼容性最好）</option>
            <option value="fileClipboard">文件剪贴板（CF_HDROP）</option>
          </select>
        </label>
        <label className="row">
          Obsidian 仓库
          <span className="row-flex">
            <input
              value={settings.obsidianVault ?? ""}
              placeholder="未设置"
              readOnly
            />
            <button
              onClick={async () => {
                const dir = await api.pickFolder();
                if (dir) await updateSettings({ obsidianVault: dir });
              }}
            >
              选择…
            </button>
          </span>
        </label>
        <label className="row check">
          <input
            type="checkbox"
            checked={settings.autostart}
            onChange={(e) => updateSettings({ autostart: e.target.checked })}
          />
          开机自启
        </label>
        <label className="row check">
          <input
            type="checkbox"
            checked={settings.closeToTray}
            onChange={(e) => updateSettings({ closeToTray: e.target.checked })}
          />
          关闭窗口时最小化到托盘
        </label>
        <label className="row check">
          <input
            type="checkbox"
            checked={scenes.attachToForwards}
            onChange={(e) =>
              updateScenes({ ...scenes, attachToForwards: e.target.checked })
            }
          />
          转发时附带场景提示词
        </label>
      </section>

      <section className="card">
        <h2>悬浮球</h2>
        <label className="row">
          悬浮球大小
          <span className="row-flex">
            <input
              type="range"
              min={48}
              max={120}
              value={settings.ballSize}
              onChange={(e) =>
                updateSettings({ ballSize: Number(e.target.value) })
              }
            />
            <span className="pill">{settings.ballSize}px</span>
          </span>
        </label>
        <label className="row">
          悬浮球颜色
          <span className="row-flex">
            {["#07c160", "#0a84ff", "#ff9f0a", "#ef4444", "#8e8e93"].map(
              (c) => (
                <button
                  key={c}
                  className="swatch"
                  style={{
                    background: c,
                    outline:
                      settings.ballColor.toLowerCase() === c
                        ? "2px solid #fff"
                        : "none",
                  }}
                  onClick={() => updateSettings({ ballColor: c })}
                  aria-label={c}
                />
              ),
            )}
            <input
              type="color"
              value={settings.ballColor}
              onChange={(e) => updateSettings({ ballColor: e.target.value })}
            />
          </span>
        </label>
        <div className="row-flex" style={{ gap: 8, marginTop: 4 }}>
          <button
            onClick={async () => {
              await api.showFloatBall();
              showToast("悬浮球已显示");
            }}
          >
            显示悬浮球
          </button>
          <button
            onClick={async () => {
              await api.hideFloatBall();
              showToast("悬浮球已隐藏（托盘可找回）");
            }}
          >
            隐藏悬浮球
          </button>
        </div>
        <p className="scene-summary" style={{ marginTop: 8 }}>
          拖拽移动，松手自动贴边 · 右键快捷转发最近批次 / 调大小 / 隐藏 · 双击打开主窗口 · 空闲自动半透明
        </p>
      </section>

      <section className="card">
        <h2>转发目标</h2>
        {targets.map((t, i) => (
          <div className="target-row" key={t.id}>
            <label className="check">
              <input
                type="checkbox"
                checked={t.enabled}
                onChange={(e) => {
                  const next = [...targets];
                  next[i] = { ...t, enabled: e.target.checked };
                  updateTargets(next);
                }}
              />
              <b>{t.displayName}</b>
            </label>
            <span className={`pill ${t.running ? "ok" : ""}`}>
              {t.id === "clipboard"
                ? "系统"
                : t.running
                  ? "运行中"
                  : "未运行"}
            </span>
            {t.pathOnly && <span className="pill">终端·路径文本</span>}
            <input
              className="exe-input"
              placeholder="自定义 exe 路径（可选）"
              value={t.exePath ?? ""}
              onChange={(e) => {
                const next = [...targets];
                next[i] = { ...t, exePath: e.target.value || null };
                updateTargets(next);
              }}
            />
          </div>
        ))}
      </section>

      <section className="card">
        <h2>场景</h2>
        {scenes.scenes.map((s) => (
          <div className="scene-row" key={s.id}>
            <label className="check">
              <input
                type="checkbox"
                checked={s.enabled}
                onChange={(e) => {
                  const next = {
                    ...scenes,
                    scenes: scenes.scenes.map((x) =>
                      x.id === s.id ? { ...x, enabled: e.target.checked } : x,
                    ),
                  };
                  updateScenes(next);
                }}
              />
              <b>{s.name}</b>
              {s.isOfficial && <span className="pill">官方</span>}
            </label>
            <p className="scene-summary">{s.summary}</p>
            <label className="check small">
              <input
                type="radio"
                name="defaultScene"
                checked={scenes.defaultSceneId === s.id}
                onChange={() =>
                  updateScenes({ ...scenes, defaultSceneId: s.id })
                }
              />
              设为默认场景
            </label>
          </div>
        ))}
      </section>

      <section className="card">
        <h2>技能中心</h2>
        <button onClick={installSkill}>安装 SKILL 文件…</button>
        {skills.length === 0 && <p className="empty">尚未安装技能</p>}
        {skills.map((s) => (
          <div className="skill-row" key={s.id}>
            <b>{s.name}</b>
            <span className="scene-summary">{s.description}</span>
            {s.official && <span className="pill">官方</span>}
            <button
              className="link-btn danger"
              onClick={async () => {
                await api.removeSkill(s.id);
                setSkills(await api.listSkills());
              }}
            >
              删除
            </button>
          </div>
        ))}
      </section>

      <section className="card about">
        <h2>关于</h2>
        <p>
          微信流 WeChatBridge for Windows · v0.1.0
          <br />
          基于开源项目{" "}
          <a href="https://github.com/freestylefly/WeChatBridge" target="_blank">
            freestylefly/WeChatBridge
          </a>{" "}
          （MIT）重写。聊天内容只来自用户主动导出的文件，全部数据保存在本机。
        </p>
      </section>

      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}
