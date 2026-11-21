import { invoke, isTauri } from '@tauri-apps/api/core';

export const isDesktop = isTauri();

export interface StorageInfo {
  schemaVersion: number;
}

export function getStorageInfo(): Promise<StorageInfo> {
  return invoke<StorageInfo>('storage_info');
}
