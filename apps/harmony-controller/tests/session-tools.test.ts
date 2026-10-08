import assert from 'node:assert/strict';
import test from 'node:test';
import { CanvasInsets, canvasViewportAboveKeyboard, remainingCanvasInsets } from '../entry/src/main/ets/input/CanvasModels.ts';
import { SessionOrbDrag, sessionToolsLayout } from '../entry/src/main/ets/model/SessionToolsModels.ts';

test('floating tools stay inside system insets in portrait, landscape and short windows', () => {
  const insets = new CanvasInsets();
  insets.left = 24; insets.right = 8; insets.top = 28; insets.bottom = 20;
  for (const [width, height] of [[390, 844], [844, 390], [320, 240]]) {
    for (const normalizedY of [0, 0.5, 1]) {
      for (const details of [false, true]) {
        const layout = sessionToolsLayout(width, height, insets, normalizedY, details);
        assert.ok(layout.orbX >= insets.left && layout.orbX + 44 <= width - insets.right);
        assert.ok(layout.orbY >= insets.top && layout.orbY + 44 <= height - insets.bottom);
        assert.ok(layout.panelX >= insets.left && layout.panelX + layout.panelWidth < layout.orbX);
        assert.ok(layout.panelY >= insets.top && layout.panelY + layout.panelHeight <= height - insets.bottom);
      }
    }
  }
});

test('a tap does not move the orb; vertical dragging clamps and ignores other fingers', () => {
  const layout = sessionToolsLayout(390, 844, new CanvasInsets(), 0.5, false);
  const drag = new SessionOrbDrag();
  drag.begin(7, 400, layout.orbY);
  assert.equal(drag.move(7, 403, layout), undefined);
  assert.equal(drag.move(8, 700, layout), undefined);
  assert.equal(drag.move(7, -500, layout), 0);
  assert.equal(drag.move(7, 1500, layout), 1);
  drag.end(7);
  assert.equal(drag.move(7, 400, layout), undefined);
  drag.begin(8, 400, layout.orbY);
  drag.cancel();
  assert.equal(drag.move(8, 600, layout), undefined);
});

test('rotation preserves the orb relative vertical position', () => {
  const insets = new CanvasInsets();
  const portrait = sessionToolsLayout(390, 844, insets, 0.8, false);
  const landscape = sessionToolsLayout(844, 390, insets, 0.8, false);
  assert.equal((portrait.orbY - portrait.minY) / (portrait.maxY - portrait.minY), 0.8);
  assert.equal((landscape.orbY - landscape.minY) / (landscape.maxY - landscape.minY), 0.8);
});

test('docked keyboard reduces the viewport once and leaves only intersecting system insets', () => {
  const physical = { x: 0, y: 24, width: 390, height: 820 };
  const keyboard = { kind: 'keyboard', edge: 'bottom', x: 0, y: 544, width: 390, height: 300 };
  const viewport = canvasViewportAboveKeyboard(physical, [keyboard]);
  assert.deepEqual(viewport, { x: 0, y: 24, width: 390, height: 520 });
  const insets = remainingCanvasInsets(viewport, [
    { kind: 'system', edge: 'top', x: 0, y: 0, width: 390, height: 32 },
    { kind: 'system', edge: 'bottom', x: 0, y: 824, width: 390, height: 20 }
  ]);
  assert.equal(insets.top, 8);
  assert.equal(insets.bottom, 0);
  assert.equal(physical.height, 820);
});

test('floating or hidden keyboards with no docked avoid area preserve the full viewport', () => {
  const physical = { x: 0, y: 0, width: 844, height: 390 };
  assert.deepEqual(canvasViewportAboveKeyboard(physical, []), physical);
  assert.deepEqual(canvasViewportAboveKeyboard(physical, [
    { kind: 'keyboard', edge: 'bottom', x: 0, y: 0, width: 0, height: 0 },
    { kind: 'system', edge: 'bottom', x: 0, y: 370, width: 844, height: 20 }
  ]), physical);
});

test('keyboard and tools buttons stay separate and above a docked keyboard at each drag limit', () => {
  for (const [width, height] of [[390, 420], [844, 180]]) {
    for (const position of [0, 0.5, 1]) {
      const layout = sessionToolsLayout(width, height, new CanvasInsets(), position, false);
      assert.equal(layout.keyboardX, layout.orbX);
      assert.ok(layout.keyboardY >= layout.orbY + 44 + 8);
      assert.ok(layout.keyboardY + 44 <= height - 12);
      assert.ok(layout.panelX + layout.panelWidth < layout.keyboardX);
      assert.ok(layout.panelY + layout.panelHeight <= height - 12);
    }
  }
});
