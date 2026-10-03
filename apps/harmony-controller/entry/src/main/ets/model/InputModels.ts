import type { ScreenState } from './DeskModels';

export interface SystemInputCommand {
  kind: string;
  x?: number;
  y?: number;
  button?: string;
  down?: boolean;
  dx?: number;
  dy?: number;
  code?: string;
  text?: string;
}

export class PointerDelta {
  dx: number = 0;
  dy: number = 0;
}

export function canSendSystemInput(screen: ScreenState, visible: boolean, controlMode: boolean): boolean {
  return visible && controlMode && screen.approved && screen.inputSupported && screen.inputGranted &&
    screen.phase === 'viewing' && screen.rendered > 0;
}

export function normalizePointerDelta(dx: number, dy: number, viewWidth: number, viewHeight: number): PointerDelta {
  const delta = new PointerDelta();
  if (!Number.isFinite(dx) || !Number.isFinite(dy) || !Number.isFinite(viewWidth) ||
    !Number.isFinite(viewHeight) || viewWidth <= 0 || viewHeight <= 0) { return delta; }
  delta.dx = Math.sign(dx) * Math.min(65535, Math.round(Math.abs(dx) * 65535 / viewWidth));
  delta.dy = Math.sign(dy) * Math.min(65535, Math.round(Math.abs(dy) * 65535 / viewHeight));
  return delta;
}

export function canSubmitInputText(value: string, previewActive: boolean): boolean {
  if (previewActive || value.length === 0) { return false; }
  let bytes = 0;
  for (let index = 0; index < value.length; index++) {
    const unit = value.charCodeAt(index);
    if (unit === 0) { return false; }
    if (unit < 0x80) { bytes++; }
    else if (unit < 0x800) { bytes += 2; }
    else if (unit >= 0xD800 && unit <= 0xDBFF && index + 1 < value.length &&
      value.charCodeAt(index + 1) >= 0xDC00 && value.charCodeAt(index + 1) <= 0xDFFF) {
      bytes += 4;
      index++;
    } else if (unit >= 0xD800 && unit <= 0xDFFF) { return false; }
    else { bytes += 3; }
    if (bytes > 512) { return false; }
  }
  return true;
}
