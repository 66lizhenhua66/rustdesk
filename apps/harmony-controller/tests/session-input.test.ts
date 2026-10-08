import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';
import { registerHooks } from 'node:module';
import { ScreenState } from '../entry/src/main/ets/model/DeskModels.ts';
import type { SystemInputCommand } from '../entry/src/main/ets/model/InputModels.ts';

registerHooks({ resolve(specifier, context, nextResolve) {
  return nextResolve(specifier.startsWith('.') && !/\.[a-z]+$/i.test(specifier) ? specifier + '.ts' : specifier, context);
} });
const { SessionInputController } = await import('../entry/src/main/ets/input/SessionInputController.ts');
const { CanvasState, CanvasInsets, remainingCanvasInsets } = await import('../entry/src/main/ets/input/CanvasModels.ts');

function setup(initialControl = true, accessMode = '') {
  const events: Record<string, unknown>[] = [];
  const commands: SystemInputCommand[] = [];
  const enabled: boolean[] = [];
  const canvas = new CanvasState();
  canvas.imageWidth = 352; canvas.imageHeight = 176;
  canvas.imageX = 24; canvas.imageY = 72;
  canvas.safeX = 24; canvas.safeY = 64; canvas.safeWidth = 352; canvas.safeHeight = 192;
  let releases = 0;
  let failure = 0;
  const input = new SessionInputController({
    send: (command: SystemInputCommand) => { commands.push(command); return failure; },
    sendEvent: (json: string) => {
      const event = JSON.parse(json);
      events.push(event);
      if (event.kind === 'configure') { canvas.inputEnabled = event.enabled; }
      return failure;
    },
    reset: () => { canvas.inputEnabled = false; canvas.nextTickMs = 0; },
    setEnabled: (value: boolean) => { enabled.push(value); return 0; },
    release: () => { releases++; canvas.nextTickMs = 0; canvas.dragReady = false; },
    focus: () => {},
    openKeyboard: () => input.keyboardChanged(true, ''),
    closeKeyboard: () => input.keyboardChanged(false, ''),
    canvasState: () => JSON.stringify(canvas),
    changed: () => {}
  }, initialControl, accessMode);
  const screen = new ScreenState();
  screen.phase = 'viewing'; screen.approved = true; screen.inputSupported = true;
  screen.rendered = 1; screen.code = 'INPUT_STATE'; screen.width = 200; screen.height = 100;
  input.setViewport(400, 300);
  input.update(screen, true);
  return { input, screen, events, enabled, canvas, commands, releases: () => releases,
    grant: () => { screen.inputGranted = true; input.update(screen, true); },
    fail: () => { failure = 3; } };
}

test('Harmony sends the same viewport event contract accepted by the real Rust canvas fixture', () => {
  const fixture = JSON.parse(readFileSync(new URL('./fixtures/canvas-events.json', import.meta.url), 'utf8'));
  const h = setup(); h.grant(); h.events.length = 0;
  const insets = new CanvasInsets(); insets.top = 40; insets.bottom = 20;
  h.input.setViewport(400, 300, insets);
  for (const event of fixture.events.slice(1)) { h.input.touch(event); }
  assert.deepEqual(h.events, fixture.events);
  assert.equal(h.input.state.canvas.imageX, 24);
  h.input.mouse({ action: 'press', button: 'left', x: 200, y: 160 });
  assert.deepEqual(h.events.at(-1), { kind: 'mouse', action: 'press', button: 'left', x: 200, y: 160 });
  h.input.wheel({ action: 'update', x: 200, y: 160, dx: 0, dy: -1, discrete: true });
  assert.deepEqual(h.events.at(-1), { kind: 'wheel', action: 'update', x: 200, y: 160, dx: 0, dy: -1, discrete: true });
  h.input.toggleKeyboard();
  assert.equal(h.input.key({ action: 'down', physicalCode: 2017, code: 'KeyA' }), false);
  h.input.send({ kind: 'text', text: '确认文字🙂' });
  assert.deepEqual(h.events.at(-1), { kind: 'text', text: '确认文字🙂' });
  h.input.blur();
  assert.equal(h.input.state.keyboardOpen, false);
  assert.equal(h.input.state.controlMode, true);
});

test('view-only can pan, zoom and reset locally while keys and text remain gated', () => {
  for (const mode of ['', 'pair', 'unattended']) {
    const h = setup(false, mode);
    assert.deepEqual(h.enabled, []);
    assert.equal(h.input.state.canvas.inputEnabled, false);
    assert.equal(h.input.touch({ action: 'down', points: [], changedPoints: [], time: 0 }), true);
    h.input.resetView();
    assert.deepEqual(h.events.at(-1), { kind: 'reset_view' });
    assert.equal(h.input.send({ kind: 'text', text: '不能发往远端' }), false);
    assert.equal(h.input.key({ action: 'down', physicalCode: 1, code: 'KeyA' }), false);
  }
  const h = setup(); h.grant();
  h.canvas.zoom = 4; h.canvas.resetVisible = true;
  h.input.setViewport(400, 300);
  h.input.selectControl(false);
  assert.equal(h.input.state.canvas.zoom, 4);
  assert.equal(h.input.state.ready, false);
  assert.deepEqual(h.enabled, [true, false]);
  const count = h.events.length;
  h.input.update(h.screen, false);
  assert.equal(h.input.touch({ action: 'down', points: [], changedPoints: [], time: 0 }), false);
  assert.equal(h.events.length, count);
});

test('native failure stops input and pending long-press timers never survive blur or hiding', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout', 'Date'], now: 100 });
  const h = setup(); h.grant();
  h.canvas.nextTickMs = 1100;
  h.input.touch({ action: 'down', points: [{ id: 1, x: 200, y: 160 }], changedPoints: [{ id: 1, x: 200, y: 160 }], time: 100 });
  h.input.blur();
  const before = h.events.length;
  t.mock.timers.tick(1001);
  assert.equal(h.events.length, before);
  h.canvas.nextTickMs = 2200;
  h.input.touch({ action: 'down', points: [], changedPoints: [], time: 1101 });
  h.input.update(h.screen, false);
  const hiddenCount = h.events.length;
  t.mock.timers.tick(2000);
  assert.equal(h.events.length, hiddenCount);
  const failed = setup(); failed.grant(); failed.fail();
  failed.input.mouse({ action: 'move', button: '', x: 200, y: 160 });
  assert.equal(failed.input.state.controlMode, false);
  assert.match(failed.input.state.notice, /暂停控制/);
  const count = failed.events.length;
  assert.equal(failed.input.key({ action: 'down', physicalCode: 1, code: 'KeyA' }), false);
  assert.equal(failed.events.length, count);
});

test('safe-area overlap avoids double subtraction and preserves uncovered cutout and keyboard edges', () => {
  const regions = [
    { edge: 'top', x: 0, y: 0, width: 800, height: 24 },
    { edge: 'left', x: 0, y: 0, width: 40, height: 480 },
    { edge: 'bottom', x: 0, y: 280, width: 800, height: 200 },
    { edge: 'bottom', x: 0, y: 464, width: 800, height: 16 }
  ];
  const actual = remainingCanvasInsets({ x: 0, y: 24, width: 800, height: 440 }, regions);
  assert.deepEqual({ ...actual }, { left: 40, top: 0, right: 0, bottom: 184 });
  const alreadySafe = remainingCanvasInsets({ x: 40, y: 24, width: 760, height: 256 }, regions);
  assert.deepEqual({ ...alreadySafe }, { left: 0, top: 0, right: 0, bottom: 0 });
});
