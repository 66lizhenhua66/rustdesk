export class CanvasInsets {
  left: number = 0;
  top: number = 0;
  right: number = 0;
  bottom: number = 0;
}

export class CanvasState {
  zoom: number = 1;
  imageX: number = 0;
  imageY: number = 0;
  imageWidth: number = 0;
  imageHeight: number = 0;
  safeX: number = 0;
  safeY: number = 0;
  safeWidth: number = 0;
  safeHeight: number = 0;
  dragReady: boolean = false;
  resetVisible: boolean = false;
  inputEnabled: boolean = false;
  gesture: string = 'idle';
  touchMode: string = 'direct';
  nextTickMs: number = 0;
}

export interface CanvasRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface CanvasAvoidRegion extends CanvasRect {
  edge: string;
  kind?: string;
}

export function canvasViewportAboveKeyboard(viewport: CanvasRect, regions: CanvasAvoidRegion[]): CanvasRect {
  let height = viewport.height;
  for (const rect of regions) {
    if (rect.kind !== 'keyboard' || rect.edge !== 'bottom' || rect.width <= 0 || rect.height <= 0 ||
      Math.min(viewport.x + viewport.width, rect.x + rect.width) <= Math.max(viewport.x, rect.x) ||
      rect.y + rect.height <= viewport.y) { continue; }
    height = Math.min(height, Math.max(0, rect.y - viewport.y));
  }
  return { x: viewport.x, y: viewport.y, width: viewport.width, height: height };
}

// Both the viewport and OS rectangles use window-local logical coordinates.
export function remainingCanvasInsets(viewport: CanvasRect, regions: CanvasAvoidRegion[]): CanvasInsets {
  const result = new CanvasInsets();
  for (const rect of regions) {
    if (rect.width <= 0 || rect.height <= 0 ||
      Math.min(viewport.x + viewport.width, rect.x + rect.width) <= Math.max(viewport.x, rect.x) ||
      Math.min(viewport.y + viewport.height, rect.y + rect.height) <= Math.max(viewport.y, rect.y)) { continue; }
    if (rect.edge === 'left') { result.left = Math.max(result.left, rect.x + rect.width - viewport.x); }
    if (rect.edge === 'top') { result.top = Math.max(result.top, rect.y + rect.height - viewport.y); }
    if (rect.edge === 'right') { result.right = Math.max(result.right, viewport.x + viewport.width - rect.x); }
    if (rect.edge === 'bottom') { result.bottom = Math.max(result.bottom, viewport.y + viewport.height - rect.y); }
  }
  result.left = Math.min(viewport.width, result.left);
  result.right = Math.min(viewport.width, result.right);
  result.top = Math.min(viewport.height, result.top);
  result.bottom = Math.min(viewport.height, result.bottom);
  return result;
}

export function decodeCanvasState(raw: string): CanvasState {
  const state = JSON.parse(raw) as CanvasState;
  const values: number[] = [state.zoom, state.imageX, state.imageY, state.imageWidth, state.imageHeight,
    state.safeX, state.safeY, state.safeWidth, state.safeHeight, state.nextTickMs];
  if (!values.every((value: number) => typeof value === 'number' && Number.isFinite(value)) ||
    state.zoom < 1 || state.zoom > 10 || state.imageWidth < 0 || state.imageHeight < 0 ||
    state.safeWidth < 0 || state.safeHeight < 0 || state.nextTickMs < 0 ||
    typeof state.dragReady !== 'boolean' || typeof state.resetVisible !== 'boolean' ||
    typeof state.inputEnabled !== 'boolean' || typeof state.gesture !== 'string' ||
    (state.touchMode !== 'direct' && state.touchMode !== 'pointer')) {
    throw new Error('Invalid canvas snapshot');
  }
  return state;
}
