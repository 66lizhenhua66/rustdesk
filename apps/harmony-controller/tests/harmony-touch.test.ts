import assert from 'node:assert/strict';
import test from 'node:test';
import { activeHarmonyTouches } from '../entry/src/main/ets/input/HarmonyTouchEvents.ts';

test('lifting both fingers ends a pinch before IDs are reused by the next gesture', () => {
  const first = { id: 0, x: 100, y: 150 };
  const second = { id: 1, x: 300, y: 150 };
  assert.deepEqual(activeHarmonyTouches('up', [first, second], [second]), [first]);
  assert.deepEqual(activeHarmonyTouches('up', [first], [first]), []);
  assert.deepEqual(activeHarmonyTouches('down', [first], [first]), [first]);
  assert.deepEqual(activeHarmonyTouches('down', [first, second], [second]), [first, second]);
});

test('single-finger tap or drag releases its last point; cancellation leaves no held points', () => {
  const point = { id: 7, x: 180, y: 120 };
  assert.deepEqual(activeHarmonyTouches('move', [point], [point]), [point]);
  assert.deepEqual(activeHarmonyTouches('up', [point], [point]), []);
  assert.deepEqual(activeHarmonyTouches('cancel', [point], [point]), []);
});
