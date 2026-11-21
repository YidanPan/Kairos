import { invoke, isTauri } from '@tauri-apps/api/core';
import type { Board, TaskAction } from './taskModel';

export const isDesktop = isTauri();

export interface StorageInfo {
  schemaVersion: number;
}

export function getStorageInfo(): Promise<StorageInfo> {
  return invoke<StorageInfo>('storage_info');
}

export function taskBoard(action?: TaskAction): Promise<Board> {
  return invoke<Board>('task_board', { action: action ?? null });
}
