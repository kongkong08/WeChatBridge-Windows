import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  BatchManifest,
  ForwardResult,
  Preview,
  SceneSettings,
  SkillInfo,
  TargetStatus,
} from "./types";

// 批次
export const importZip = (path: string) =>
  invoke<BatchManifest>("import_zip", { path });

export const listBatches = () => invoke<BatchManifest[]>("list_batches");

export const deleteBatch = (batchId: string) =>
  invoke<void>("delete_batch", { batchId });

export const previewBatch = (batchId: string) =>
  invoke<Preview>("preview_batch", { batchId });

export const openBatchFolder = (batchId: string) =>
  invoke<void>("open_batch_folder", { batchId });

// 转发
export const listTargets = () => invoke<TargetStatus[]>("list_targets");

export const forwardBatch = (
  batchId: string,
  targetId: string,
  sceneId: string | null,
) => invoke<ForwardResult>("forward_batch", { batchId, targetId, sceneId });

export const copyManualPayload = (
  batchId: string,
  targetId: string,
  sceneId: string | null,
) => invoke<string>("copy_manual_payload", { batchId, targetId, sceneId });

export const exportObsidian = (batchId: string) =>
  invoke<string>("export_obsidian", { batchId });

// 场景
export const listScenes = () => invoke<SceneSettings>("list_scenes");

export const saveScenes = (settings: SceneSettings) =>
  invoke<void>("save_scenes", { settings });

// 设置
export const getSettings = () => invoke<AppSettings>("get_settings");

export const saveSettings = (settings: AppSettings) =>
  invoke<void>("save_settings", { settings });

export const saveTargets = (targets: TargetStatus[]) =>
  invoke<void>("save_targets", { targets });

export const pickFile = (filters: [string, string[]][]) =>
  invoke<string | null>("pick_file", { filters });

export const pickFolder = () => invoke<string | null>("pick_folder");

// 技能中心
export const listSkills = () => invoke<SkillInfo[]>("list_skills");

export const installSkill = (path: string) =>
  invoke<SkillInfo>("install_skill", { path });

export const removeSkill = (skillId: string) =>
  invoke<void>("remove_skill", { skillId });
