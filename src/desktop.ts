import { invoke, isTauri } from '@tauri-apps/api/core';
import type { Board, TaskAction } from './taskModel';

export const isDesktop = isTauri();

export interface DailyGreeting {
  date: string;
  text: string;
}

export interface StorageInfo {
  schemaVersion: number;
  dailyGreeting: DailyGreeting | null;
}

export type WindowMode = 'smart' | 'desktop' | 'alwaysOnTop';
export type StartupPage = 'tasks' | 'completed' | 'statistics';

export interface Behavior {
  windowMode: WindowMode;
  startupPage: StartupPage;
  whitelist: string[];
}

export interface Integration {
  preferences: Behavior;
  clickThrough: boolean;
  startup: boolean | null;
  unavailableShortcuts: string[];
}

export const integrationStatus = () => invoke<Integration>('integration_status');
export const saveBehavior = (preferences: Behavior) => invoke<Behavior>('save_behavior', { preferences });
export const setLaunchAtLogin = (enabled: boolean) => invoke<boolean>('set_launch_at_login', { enabled });
export const setClickThrough = (enabled: boolean) => invoke<boolean>('set_click_through', { enabled });
export const saveDailyGreeting = (text: string) => invoke<DailyGreeting | null>('save_daily_greeting', { text });

export function getStorageInfo(): Promise<StorageInfo> {
  return invoke<StorageInfo>('storage_info');
}

export function taskBoard(action?: TaskAction): Promise<Board> {
  return invoke<Board>('task_board', { action: action ?? null });
}
