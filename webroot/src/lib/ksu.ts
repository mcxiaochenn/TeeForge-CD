/**
 * KernelSU WebUI 桥接层 [KernelSU WebUI bridge]
 *
 * 只暴露最小能力：执行命令、流式执行、读取应用信息、轻提示。
 * 所有调用都先判断宿主是否存在，避免在普通浏览器里抛异常。
 */

export const MODDIR = '/data/adb/modules/teeforge_cd';
export const DATA_DIR = '/data/adb/teeforge';
export const CONFIG = DATA_DIR + '/config.conf';
export const TEEFORGE = MODDIR + '/teeforge';
export const SELECTION_FILE = DATA_DIR + '/target-selection.json';

export interface ExecResult {
  code: number;
  stdout: string;
  stderr: string;
}

interface KsuHost {
  exec(command: string, options: string, callback: string): void;
  spawn?(command: string, options?: { cwd?: string }): SpawnChild;
  toast?(message: string): void;
  listPackages?(type: string): string[];
  getPackagesInfo?(packages: string[]): { packageName: string; appLabel: string }[];
}

export interface SpawnChild {
  stdout: { on(event: 'data', cb: (data: string) => void): void };
  stderr: { on(event: 'data', cb: (data: string) => void): void };
  on(event: 'exit', cb: (code: number) => void): void;
  on(event: 'error', cb: (error: unknown) => void): void;
}

function host(): KsuHost | null {
  const candidate = (window as unknown as { ksu?: KsuHost }).ksu;
  return candidate && typeof candidate.exec === 'function' ? candidate : null;
}

export function hasHost(): boolean {
  return host() !== null;
}

export function canSpawn(): boolean {
  return typeof host()?.spawn === 'function';
}

export function exec(command: string, options = '{}'): Promise<ExecResult> {
  return new Promise((resolve, reject) => {
    const bridge = host();
    if (!bridge) {
      reject(new Error('KernelSU bridge unavailable'));
      return;
    }
    const callback = `tf_cb_${Date.now()}_${Math.floor(Math.random() * 10000)}`;
    (window as unknown as Record<string, unknown>)[callback] = (
      code: number,
      stdout: string,
      stderr: string,
    ) => {
      delete (window as unknown as Record<string, unknown>)[callback];
      resolve({ code, stdout: stdout || '', stderr: stderr || '' });
    };
    bridge.exec(command, options, callback);
  });
}

export function spawn(command: string): SpawnChild | null {
  const bridge = host();
  if (!bridge || typeof bridge.spawn !== 'function') return null;
  try {
    return bridge.spawn(command, { cwd: MODDIR });
  } catch {
    return null;
  }
}

export function toast(message: string): void {
  host()?.toast?.(message);
}

/** 应用名与图标的可选增强；接口不存在时由调用方回退到包名。 */
export async function appLabels(packages: string[]): Promise<Record<string, string>> {
  const bridge = host();
  if (!bridge?.getPackagesInfo || packages.length === 0) return {};
  try {
    const info = bridge.getPackagesInfo(packages);
    const map: Record<string, string> = {};
    for (const item of info) {
      if (item.appLabel) map[item.packageName] = item.appLabel;
    }
    return map;
  } catch {
    return {};
  }
}

export function iconUrl(packageName: string): string {
  return `ksu://icon/${packageName}`;
}
