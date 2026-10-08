import { UIContext, window } from '@kit.ArkUI';
import { CanvasAvoidRegion } from '../input/CanvasModels';

export class CanvasAvoidArea {
  private ui: UIContext;
  private changed: (regions: CanvasAvoidRegion[], error: string) => void;
  private owner?: window.Window;
  private active: boolean = false;
  private onAvoidChanged = (_event: window.AvoidAreaOptions): void => { this.refresh(); };

  constructor(ui: UIContext, changed: (regions: CanvasAvoidRegion[], error: string) => void) {
    this.ui = ui;
    this.changed = changed;
  }

  async start(): Promise<void> {
    this.active = true;
    try {
      const context = this.ui.getHostContext();
      if (!context) { throw new Error('Host context unavailable'); }
      const owner = await window.getLastWindow(context);
      if (!this.active) { return; }
      this.owner = owner;
      owner.on('avoidAreaChange', this.onAvoidChanged);
      this.refresh();
    } catch {
      if (this.active) { this.changed([], '系统安全区域读取失败，请重新进入会话。'); }
    }
  }

  refresh(): void {
    if (!this.active || !this.owner) { return; }
    try {
      const regions: CanvasAvoidRegion[] = [];
      for (const type of [window.AvoidAreaType.TYPE_SYSTEM, window.AvoidAreaType.TYPE_CUTOUT,
        window.AvoidAreaType.TYPE_NAVIGATION_INDICATOR, window.AvoidAreaType.TYPE_KEYBOARD]) {
        const area = this.owner.getWindowAvoidArea(type);
        this.add(regions, 'left', area.leftRect);
        this.add(regions, 'top', area.topRect);
        this.add(regions, 'right', area.rightRect);
        this.add(regions, 'bottom', area.bottomRect);
      }
      this.changed(regions, '');
    } catch {
      this.changed([], '系统安全区域更新失败，请重新进入会话。');
    }
  }

  stop(): void {
    this.active = false;
    try { this.owner?.off('avoidAreaChange', this.onAvoidChanged); }
    catch { this.changed([], '系统安全区域监听未能移除。'); }
    this.owner = undefined;
  }

  private add(regions: CanvasAvoidRegion[], edge: string, rect: window.Rect): void {
    if (rect.width <= 0 || rect.height <= 0) { return; }
    regions.push({ edge: edge, x: this.ui.px2vp(rect.left), y: this.ui.px2vp(rect.top),
      width: this.ui.px2vp(rect.width), height: this.ui.px2vp(rect.height) });
  }
}
