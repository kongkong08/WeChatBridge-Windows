// 与 Rust 端 DTO 对齐的类型定义（serde camelCase）。

export type ShareAction =
  | "codex" | "claude" | "doubao" | "qwen" | "workBuddy"
  | "weSight" | "obsidian" | "clipboard" | "custom";

export type BatchState = "received" | "delivered" | "copied" | "failed" | "expired";

export interface BatchManifest {
  id: string;
  createdAt: string; // "YYYY-MM-DDTHH:mm:ss"
  action: ShareAction | null;
  displayName: string;
  relativePath: string;
  state: BatchState;
  chatName: string | null;
  sceneName: string | null;
  error: string | null;
}

export interface TranscriptRecordDto {
  sender: string;
  date: string;
  text: string;
}

export interface Preview {
  chatName: string | null;
  messageCount: number;
  records: TranscriptRecordDto[];
  attachments: string[];
  transcriptPath: string | null;
}

export interface TargetApp {
  id: string;
  displayName: string;
  processNames: string[];
  windowTitleKeywords: string[];
  exePath: string | null;
  pathOnly: boolean;
  builtin: boolean;
  enabled: boolean;
}

export interface TargetStatus extends TargetApp {
  running: boolean;
}

export interface WeChatScene {
  id: string;
  name: string;
  summary: string;
  instruction: string;
  outputSpec: string;
  keywords: string[];
  enabled: boolean;
  packageVersion: string;
  author: string;
  applicability: string;
  requiredSkillIds: string[];
  compatibleAgents: string[];
  isOfficial: boolean;
}

export interface SceneSettings {
  scenes: WeChatScene[];
  defaultSceneId: string | null;
  attachToForwards: boolean;
}

export type PasteMode = "textWithPaths" | "fileClipboard";

export interface AppSettings {
  language: string;
  pasteMode: PasteMode;
  obsidianVault: string | null;
  autostart: boolean;
  closeToTray: boolean;
  ballSize: number;
  ballColor: string;
  autoForwardTarget: string | null;
}

export interface SkillInfo {
  id: string;
  name: string;
  description: string;
  official: boolean;
}

export interface ForwardResult {
  manifest: BatchManifest;
  usedFallbackCopy: boolean;
  message: string;
}
