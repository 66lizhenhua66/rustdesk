import assert from 'node:assert/strict';
import test from 'node:test';
import { ScreenState } from '../entry/src/main/ets/model/DeskModels.ts';
import { normalizePointerDelta, canSendSystemInput, canSubmitInputText } from '../entry/src/main/ets/model/InputModels.ts';

test('input requires visible rendered video, independent permission and local control mode', () => {
  const screen = new ScreenState();
  screen.phase = 'viewing';
  screen.approved = true;
  screen.rendered = 1;
  assert.equal(canSendSystemInput(screen, true, true), false);
  screen.inputSupported = true;
  screen.inputGranted = true;
  assert.equal(canSendSystemInput(screen, true, true), true);
  assert.equal(canSendSystemInput(screen, false, true), false);
  assert.equal(canSendSystemInput(screen, true, false), false);
  screen.rendered = 0;
  assert.equal(canSendSystemInput(screen, true, true), false);
  screen.rendered = 1;
  screen.phase = 'ended';
  assert.equal(canSendSystemInput(screen, true, true), false);
});

test('touchpad sends bounded relative movement from displayed-image dimensions without tracking a cursor', () => {
  const moved = normalizePointerDelta(100, -50, 1000, 500);
  assert.equal(moved.dx, 6554);
  assert.equal(moved.dy, -6554);
  const edge = normalizePointerDelta(2000, -2000, 1000, 500);
  assert.equal(edge.dx, 65535);
  assert.equal(edge.dy, -65535);
  const invalid = normalizePointerDelta(10, 10, 0, 0);
  assert.equal(invalid.dx, 0);
  assert.equal(invalid.dy, 0);
  assert.equal(normalizePointerDelta(0, 0, 1000, 500).dx, 0);
});

test('text sends require explicit non-preview content and preserve Unicode and whitespace', () => {
  assert.equal(canSubmitInputText(' 中文\n \t🙂', false), true);
  assert.equal(canSubmitInputText('拼音候选', true), false);
  assert.equal(canSubmitInputText('', false), false);
  assert.equal(canSubmitInputText('x'.repeat(512), false), true);
  assert.equal(canSubmitInputText('x'.repeat(513), false), false);
  assert.equal(canSubmitInputText('🙂'.repeat(128), false), true);
  assert.equal(canSubmitInputText('🙂'.repeat(129), false), false);
  assert.equal(canSubmitInputText('\ud800', false), false);
  assert.equal(canSubmitInputText('before\0after', false), false);
});
