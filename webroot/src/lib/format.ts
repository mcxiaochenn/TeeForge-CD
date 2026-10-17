/** 展示层格式化 [Presentation helpers] */

export function formatBytes(bytes: number | null | undefined): string {
  if (!bytes || bytes <= 0) return '0 B';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
}

export function formatUnix(seconds: number | null | undefined): string {
  if (!seconds) return '—';
  const date = new Date(seconds * 1000);
  if (Number.isNaN(date.getTime())) return '—';
  return date.toLocaleString();
}

export function shortFingerprint(value: string, keep = 12): string {
  if (!value) return '—';
  if (value.length <= keep * 2) return value;
  return `${value.slice(0, keep)}…${value.slice(-keep)}`;
}

export function stageLabel(stage?: string): string {
  switch (stage) {
    case 'prerequisite':
      return 'prerequisite';
    case 'transport':
      return 'transport';
    case 'frame':
      return 'frame';
    case 'ta':
      return 'ta';
    case 'server':
      return 'server';
    case 'keybox':
      return 'keybox';
    case 'target':
      return 'target';
    case 'update':
      return 'update';
    default:
      return stage || 'unknown';
  }
}
