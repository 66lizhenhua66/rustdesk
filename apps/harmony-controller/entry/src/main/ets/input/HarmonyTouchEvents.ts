import type { InputPoint } from './InputEvents';

export function activeHarmonyTouches(action: string, touches: InputPoint[], changedTouches: InputPoint[]): InputPoint[] {
  if (action === 'cancel') { return []; }
  // ArkUI includes the lifted finger in touches; the shared engine expects only held fingers.
  if (action === 'up') {
    return touches.filter((point: InputPoint) =>
      !changedTouches.some((changed: InputPoint) => changed.id === point.id));
  }
  return touches;
}
