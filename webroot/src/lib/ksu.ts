export const MODDIR = '/data/adb/modules/teeforge_cd';
export const DATA_DIR = '/data/adb/teeforge';
export const CONFIG = DATA_DIR + '/config.conf';
export const TEEFORGE = MODDIR + '/teeforge';
export const EXEC_OPTIONS = { cwd: MODDIR };
export const EXEC_OPTIONS_JSON = JSON.stringify(EXEC_OPTIONS);

export interface CommandResult {
  code: number;
  stdout: string;
  stderr: string;
}

export function teeforgeCommand(flag: string): string {
  return TEEFORGE + ' --config ' + CONFIG + ' ' + flag;
}

export function execCommand(
  command: string,
  options = EXEC_OPTIONS_JSON,
): Promise<CommandResult> {
  return new Promise((resolve) => {
    const callback = 'cb_' + Date.now() + '_' + ((Math.random() * 10000) | 0);
    (window as any)[callback] = (code: number, stdout: string, stderr: string) => {
      delete (window as any)[callback];
      resolve({ code, stdout: stdout || '', stderr: stderr || '' });
    };
    ksu.exec(command, options, callback);
  });
}

export async function execText(command: string): Promise<string> {
  const result = await execCommand(command, '{}');
  if (result.code !== 0) {
    throw new Error(result.stderr || result.stdout || 'command failed');
  }
  return result.stdout;
}
