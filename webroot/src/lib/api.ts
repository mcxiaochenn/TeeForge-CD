/**
 * 命令封装 [Command wrappers]
 *
 * 所有后端调用都走 `--json`，统一解析 `{ok,data}` / `{ok,error}`。
 * 前端不拼业务逻辑，只做参数与错误搬运。
 */

import {
  CONFIG,
  SELECTION_FILE,
  TEEFORGE,
  exec,
  hasHost,
  spawn,
} from './ksu';

export interface ApiErrorPayload {
  stage?: string;
  code?: string;
  message?: string;
  hint?: string;
  raw?: unknown;
}

export class ApiError extends Error {
  stage?: string;
  code?: string;
  hint?: string;
  raw?: unknown;

  constructor(payload: ApiErrorPayload, fallback = 'command failed') {
    super(payload.message || fallback);
    this.name = 'ApiError';
    this.stage = payload.stage;
    this.code = payload.code;
    this.hint = payload.hint;
    this.raw = payload.raw;
  }
}

interface Envelope<T> {
  ok: boolean;
  data?: T;
  error?: ApiErrorPayload;
}

function command(args: string): string {
  return `${TEEFORGE} --config ${CONFIG} ${args} --json`;
}

function parseEnvelope<T>(stdout: string, exitCode: number): T {
  const text = stdout.trim();
  const start = text.indexOf('{');
  const end = text.lastIndexOf('}');
  if (start < 0 || end <= start) {
    throw new ApiError(
      { message: text || `命令无输出 [no output], exit ${exitCode}` },
      'invalid response',
    );
  }
  let envelope: Envelope<T>;
  try {
    envelope = JSON.parse(text.slice(start, end + 1)) as Envelope<T>;
  } catch {
    throw new ApiError({ message: text.slice(0, 400) }, 'invalid JSON');
  }
  if (!envelope.ok) {
    throw new ApiError(envelope.error ?? { message: 'unknown error' });
  }
  return envelope.data as T;
}

/** 一次性执行，等待结果。 */
export async function call<T>(args: string): Promise<T> {
  if (!hasHost()) throw new ApiError({ message: 'bridge-unavailable' });
  const result = await exec(command(args), JSON.stringify({ cwd: '/data/adb/modules/teeforge_cd' }));
  return parseEnvelope<T>(result.stdout, result.code);
}

/**
 * 流式执行：stderr 是进度日志，stdout 最后输出 JSON 信封。
 * 宿主没有 spawn 时退回一次性执行（没有实时日志，但结果一致）。
 */
export async function callStream<T>(
  args: string,
  onLog?: (chunk: string) => void,
): Promise<T> {
  if (!hasHost()) throw new ApiError({ message: 'bridge-unavailable' });
  const child = spawn(command(args));
  if (!child) return call<T>(args);

  return new Promise<T>((resolve, reject) => {
    let stdout = '';
    child.stdout.on('data', (chunk) => {
      stdout += chunk;
    });
    child.stderr.on('data', (chunk) => {
      onLog?.(chunk);
    });
    child.on('exit', (code) => {
      try {
        resolve(parseEnvelope<T>(stdout, code));
      } catch (error) {
        reject(error);
      }
    });
    child.on('error', (error) => {
      reject(new ApiError({ message: String(error) }));
    });
  });
}

// ---------------------------------------------------------------- 数据类型

export interface Bootstrap {
  device: {
    brand: string;
    model: string;
    name: string;
    android: string;
    sdk: string;
    security_patch: string;
    abi: string;
    soc_model: string;
    soc_manufacturer: string;
    verified_boot: string;
  };
  system: { kernel: string; selinux: string };
  module: { version: string; version_code: number };
  root: { method: string; version: string };
  config: { keybox_source: 'remote' | 'rkp'; target_mode: 'auto' | 'manual'; update_channel: string };
  capabilities: { rkp_platform: boolean; rkp_supported: boolean; rkp_artifacts: boolean };
  paths: { rkp_dir: string; keybox_dir: string };
}

export interface TargetList {
  mode: 'auto' | 'manual';
  backends: string[];
  packages: { name: string; uids: number[]; selected: boolean }[];
  counts: { installed: number; selected: number };
}

export interface ApplyResult {
  mode: string;
  selected: number;
  outcome: { state: string; packages?: number; backends?: number };
}

export type RkpState = 'Qualcomm' | 'Other' | 'Unknown' | 'Conflicting';

export interface RkpStatus {
  supported: boolean;
  platform: { os: string; arch: string; arm64_android: boolean };
  root: { method: string; version: string };
  soc: { state: RkpState; manufacturer: string; model: string; evidence: string[] };
  library: { path: string; present: boolean };
  device: { path: string; present: boolean };
  fingerprint: string;
  server: string;
  blockers: { id: string; detail: string }[];
  artifacts: { exists: boolean; dir: string; meta: Record<string, unknown> | null };
}

export interface RkpArtifacts {
  exists: boolean;
  dir: string;
  keybox: { path: string; size: number; mtime: number | null } | null;
  chain: { path: string; size: number; mtime: number | null } | null;
  meta_file: { path: string; size: number; mtime: number | null } | null;
  meta: Record<string, unknown> | null;
}

export interface UpdateInfo {
  channel: string;
  endpoint: string;
  current: { version: string; version_code: number };
  latest: {
    version: string;
    version_code: number;
    zip_url: string;
    changelog_url: string | null;
    sha256: string | null;
    size: number | null;
  };
  available: boolean;
}

export interface UpdateInstall {
  installed: boolean;
  reason?: string;
  version?: string;
  installer?: string;
  needs_reboot?: boolean;
  saved_to?: string;
}

// ---------------------------------------------------------------- 具体命令

export const api = {
  bootstrap: () => call<Bootstrap>('--bootstrap'),

  targetList: () => call<TargetList>('--target-list'),
  targetMode: (mode: 'auto' | 'manual') => call<ApplyResult>(`--target-mode ${mode}`),

  rkpStatus: () => call<RkpStatus>('--rkp-status'),
  rkpIssue: (onLog?: (chunk: string) => void) => callStream<Record<string, unknown>>('--rkp-issue', onLog),
  rkpArtifacts: () => call<RkpArtifacts>('--rkp-artifacts'),
  rkpDelete: () => call<{ deleted: boolean; freed_bytes: number }>('--rkp-delete'),

  keybox: (source: 'remote' | 'rkp', onLog?: (chunk: string) => void) =>
    callStream<{ source: string; targets: { path: string; changed: boolean }[] }>(
      `--keybox --source ${source}`,
      onLog,
    ),

  updateChannel: (channel: 'stable' | 'dev') =>
    call<{ channel: string }>(`--update-channel ${channel}`),
  updateCheck: () => callStream<UpdateInfo>('--self-update check'),
  updateInstall: (onLog?: (chunk: string) => void) =>
    callStream<UpdateInstall>('--self-update install', onLog),

  /**
   * 手动选择集合：先写文件再让后端读取，避免把几百个包名塞进命令行。
   * The selection is written as a file so argument length never becomes a limit.
   */
  async targetApply(packages: string[]): Promise<ApplyResult> {
    const payload = JSON.stringify({ version: 1, packages }, null, 0);
    const encoded = btoa(unescape(encodeURIComponent(payload)));
    const write = await exec(
      `printf '%s' '${encoded}' | base64 -d > '${SELECTION_FILE}'`,
      '{}',
    );
    if (write.code !== 0) {
      throw new ApiError({ message: write.stderr || '无法写入选择文件 [cannot write selection]' });
    }
    return call<ApplyResult>(`--target-apply ${SELECTION_FILE}`);
  },
};
