import type { CanvasInsets } from '../input/CanvasModels';

export const SESSION_ORB_SIZE: number = 44;
export const SESSION_ORB_DRAG_THRESHOLD: number = 4;

export interface SessionToolsLayout {
  orbX: number;
  orbY: number;
  minY: number;
  maxY: number;
  panelX: number;
  panelY: number;
  panelWidth: number;
  panelHeight: number;
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

export function sessionToolsLayout(width: number, height: number, insets: CanvasInsets,
  normalizedY: number, details: boolean): SessionToolsLayout {
  const left = insets.left + 12;
  const right = Math.max(left, width - insets.right - 12);
  const top = insets.top + 12;
  const bottom = Math.max(top, height - insets.bottom - 12);
  const minY = top;
  const maxY = Math.max(minY, bottom - SESSION_ORB_SIZE);
  const orbX = Math.max(left, right - SESSION_ORB_SIZE);
  const orbY = minY + (maxY - minY) * clamp(normalizedY, 0, 1);
  const panelWidth = Math.max(1, Math.min(248, orbX - left - 8));
  const panelHeight = Math.max(1, Math.min(details ? 420 : 326, bottom - top));
  return { orbX: orbX, orbY: orbY, minY: minY, maxY: maxY,
    panelX: Math.max(left, orbX - panelWidth - 8),
    panelY: clamp(orbY + SESSION_ORB_SIZE / 2 - panelHeight / 2, top, bottom - panelHeight),
    panelWidth: panelWidth, panelHeight: panelHeight };
}

export class SessionOrbDrag {
  private pointerId: number = -1;
  private startY: number = 0;
  private startTop: number = 0;
  private moved: boolean = false;

  begin(id: number, windowY: number, orbY: number): void {
    if (this.pointerId !== -1) { return; }
    this.pointerId = id;
    this.startY = windowY;
    this.startTop = orbY;
    this.moved = false;
  }

  move(id: number, windowY: number, layout: SessionToolsLayout): number | undefined {
    if (id !== this.pointerId) { return undefined; }
    const delta = windowY - this.startY;
    if (Math.abs(delta) > SESSION_ORB_DRAG_THRESHOLD) { this.moved = true; }
    if (!this.moved || layout.maxY <= layout.minY) { return undefined; }
    return (clamp(this.startTop + delta, layout.minY, layout.maxY) - layout.minY) /
      (layout.maxY - layout.minY);
  }

  end(id: number): void {
    if (id === this.pointerId) { this.pointerId = -1; }
  }

  cancel(): void { this.pointerId = -1; }
}
