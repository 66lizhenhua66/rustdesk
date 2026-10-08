import assert from 'node:assert/strict';
import test from 'node:test';
import { CanvasInsets } from '../entry/src/main/ets/input/CanvasModels.ts';
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
