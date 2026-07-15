/// <reference types="astro/client" />

/** KernelSU WebUI JS bridge — injected by KernelSU Manager at runtime */
declare const ksu: {
  exec(cmd: string, argsJson: string, callbackName: string): void;
  spawn(cmd: string, args?: string[]): {
    stdout: { on(event: 'data', cb: (data: string) => void): void };
    stderr: { on(event: 'data', cb: (data: string) => void): void };
    on(event: 'exit', cb: (code: number) => void): void;
    on(event: 'error', cb: (err: any) => void): void;
  };
  toast(message: string): void;
  fullScreen(isFullScreen: boolean): void;
  enableEdgeToEdge(enable: boolean): void;
  moduleInfo(): string;
  listPackages(type: string): string[];
  exit(): void;
};

/** TeeForge WebUI global API */
interface TeeForgeAPI {
  t(key: string): string;
  applyI18n(): void;
  getLang(): string;
  setLang(lang: string): void;
  getTheme(): string;
  getMode(): string;
  setTheme(theme: string, mode?: string): void;
  refreshI18n(): void;
}

interface TeeForgeDialog {
  open(title: string): void;
  appendLog(text: string): void;
  done(success: boolean, exitCode?: number): void;
  close(): void;
}

interface Window {
  __tf: TeeForgeAPI;
  __tfDialog: TeeForgeDialog;
}
