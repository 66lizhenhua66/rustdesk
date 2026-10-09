export class VideoPreferences {
  quality: string = 'balanced';
  fps: number = 15;
  resolutionMode: string = 'preserve';
  resolutionWidth: number = 1920;
  resolutionHeight: number = 1080;
}

export interface DisplayResolution {
  width: number;
  height: number;
}

export function validStreamResolution(width: number, height: number): boolean {
  return (width === 0 && height === 0) || (width === 1280 && height === 720) ||
    (width === 1920 && height === 1080) || (width === 2560 && height === 1440);
}

export function validDisplayResolution(width: number, height: number): boolean {
  return Number.isInteger(width) && Number.isInteger(height) && width >= 2 && height >= 2 &&
    width <= 4096 && height <= 4096 && width % 2 === 0 && height % 2 === 0 && width * height <= 8294400;
}

export function resolutionLabel(mode: string, width: number, height: number): string {
  if (width === 0 && height === 0) { return mode === 'sync' ? '恢复连接前分辨率' : '跟随远端（最高 4K）'; }
  return `${width} × ${height}`;
}

export function videoSettingsErrorLabel(code: string): string {
  switch (code) {
    case 'DISPLAY_BUSY': return '显示设置正在被另一会话使用，已保持上一设置。';
    case 'UNSUPPORTED_RESOLUTION': return '显示器不支持此分辨率，已保持上一设置。';
    case 'DISPLAY_CHANGED': return '本机显示设置已发生变化，请重新连接后再调整。';
    case 'DISPLAY_CONTROL_REQUIRED': return '请先开启控制，再修改 Windows 分辨率。';
    case 'DISPLAY_SWITCH_FAILED': return 'Windows 分辨率切换失败，已回退上一设置。';
    case 'VIDEO_REBUILD_FAILED': return '新画面准备失败，已回退上一设置。';
    default: return '';
  }
}

export function validVideoPreferences(quality: string, fps: number): boolean {
  return (quality === 'low' || quality === 'balanced' || quality === 'high') &&
    (fps === 10 || fps === 15 || fps === 30);
}

export function videoQualityLabel(quality: string): string {
  if (quality === 'low') { return '省流'; }
  if (quality === 'high') { return '清晰'; }
  return '均衡';
}

export function readVideoPreferences(raw: string): VideoPreferences {
  try {
    const saved = JSON.parse(raw) as VideoPreferences;
    if (saved && validVideoPreferences(saved.quality, saved.fps)) {
      const result = new VideoPreferences();
      result.quality = saved.quality;
      result.fps = saved.fps;
      if (saved.resolutionMode === 'preserve' && validStreamResolution(saved.resolutionWidth, saved.resolutionHeight)) {
        result.resolutionWidth = saved.resolutionWidth;
        result.resolutionHeight = saved.resolutionHeight;
      }
      return result;
    }
  } catch { /* Invalid optional preferences fall back to the default preset. */ }
  return new VideoPreferences();
}
