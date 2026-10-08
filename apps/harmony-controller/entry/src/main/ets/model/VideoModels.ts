export class VideoPreferences {
  quality: string = 'balanced';
  fps: number = 15;
}

export function validVideoPreferences(quality: string, fps: number): boolean {
  return (quality === 'low' || quality === 'balanced' || quality === 'high') &&
    (fps === 10 || fps === 15 || fps === 30);
}

export function videoQualityLabel(quality: string): string {
  if (quality === 'low') { return '省流 · 720p'; }
  if (quality === 'high') { return '高清 · 1440p'; }
  return '标准 · 1080p';
}

export function readVideoPreferences(raw: string): VideoPreferences {
  try {
    const saved = JSON.parse(raw) as VideoPreferences;
    if (saved && validVideoPreferences(saved.quality, saved.fps)) { return saved; }
  } catch { /* Invalid optional preferences fall back to the default preset. */ }
  return new VideoPreferences();
}
