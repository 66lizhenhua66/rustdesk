import assert from 'node:assert/strict';
import test from 'node:test';
import { registerHooks } from 'node:module';
import { ScreenState } from '../entry/src/main/ets/model/DeskModels.ts';
import type { SystemInputCommand } from '../entry/src/main/ets/model/InputModels.ts';

registerHooks({ resolve(specifier, context, nextResolve) {
  return nextResolve(specifier.startsWith('.') && !/\.[a-z]+$/i.test(specifier) ? specifier + '.ts' : specifier, context);
} });
const { SessionInputController } = await import('../entry/src/main/ets/input/SessionInputController.ts');

function setup(initialControl = true, accessMode = '') {
  const events: Record<string, unknown>[] = [];
  const enabled: boolean[] = [];
  let resets = 0;
  let releases = 0;
  let failure = 0;
  const input = new SessionInputController({
    send: (_command: SystemInputCommand) => failure,
    sendEvent: (json: string) => { events.push(JSON.parse(json)); return failure; },
    reset: () => { resets++; },
    setEnabled: (value: boolean) => { enabled.push(value); return 0; },
    release: () => { releases++; },
    focus: () => {},
    openKeyboard: () => input.keyboardChanged(true, ''),
    closeKeyboard: () => input.keyboardChanged(false, ''),
    canvasState: () => '{}',
    changed: () => {}
  }, initialControl, accessMode);
  const screen = new ScreenState();
  screen.phase = 'viewing'; screen.approved = true; screen.inputSupported = true;
  screen.rendered = 1; screen.code = 'INPUT_STATE';
  input.setViewport(1000, 500);
  input.update(screen, true);
  return { input, screen, events, enabled, resets: () => resets, releases: () => releases,
    grant: () => { screen.inputGranted = true; input.update(screen, true); }, fail: () => { failure = 3; } };
}

test('Harmony forwards standard events to the shared native engine after input confirmation', () => {
  const h = setup();
  assert.deepEqual(h.enabled, [true]);
  assert.equal(h.input.send({ kind: 'text', text: '未批准' }), false);
  h.screen.code = 'INPUT_ENABLING'; h.input.update(h.screen, true);
  assert.deepEqual(h.enabled, [true]);
  h.screen.code = 'INPUT_STATE'; h.grant();
  const point = { id: 1, x: 500, y: 250 };
  h.input.touch({ action: 'down', points: [point], changedPoints: [point], time: 100 });
  assert.deepEqual(h.events.at(-1), { kind: 'touch', action: 'down', points: [point], changedPoints: [point], time: 100, width: 1000, height: 500 });
  h.input.mouse({ action: 'press', button: 'left', x: 100, y: 50 });
  assert.equal(h.events.at(-1)?.kind, 'mouse');
  h.input.wheel({ action: 'update', x: 100, y: 50, dx: 0, dy: -1, discrete: true });
  assert.equal(h.events.at(-1)?.kind, 'wheel');
  h.input.key({ action: 'down', physicalCode: 2017, code: 'KeyA' });
  assert.deepEqual(h.events.at(-1), { kind: 'key', action: 'down', physicalCode: 2017, code: 'KeyA' });
  h.input.toggleKeyboard();
  assert.equal(h.input.key({ action: 'down', physicalCode: 2017, code: 'KeyA' }), false);
  const text = '确认文字🙂'.repeat(100);
  h.input.send({ kind: 'text', text });
  assert.deepEqual(h.events.at(-1), { kind: 'text', text });
  h.input.blur();
  assert.equal(h.input.state.keyboardOpen, false);
  assert.equal(h.input.state.controlMode, true);
  assert.ok(h.releases() > 0);
});

test('native failure, view-only, revocation and hidden sessions stop every input source', () => {
  const h = setup(); h.grant();
  h.input.selectControl(false);
  assert.deepEqual(h.enabled, [true, false]);
  assert.equal(h.input.send({ kind: 'text', text: '仅查看' }), false);
  h.screen.inputGranted = false; h.input.update(h.screen, true);
  h.input.selectControl(true); h.grant(); h.fail();
  h.input.touch({ action: 'down', points: [], changedPoints: [], time: 0 });
  assert.equal(h.input.state.controlMode, false);
  assert.match(h.input.state.notice, /暂停控制/);
  const count = h.events.length;
  assert.equal(h.input.key({ action: 'down', physicalCode: 1, code: 'KeyA' }), false);
  assert.equal(h.events.length, count);
  assert.ok(h.resets() > 0);
  const hidden = setup(); hidden.grant(); hidden.input.update(hidden.screen, false);
  assert.equal(hidden.input.state.ready, false);
  assert.deepEqual(hidden.enabled, [true, false]);
  const denied = setup(); denied.input.update(denied.screen, true); denied.input.update(denied.screen, true);
  assert.equal(denied.input.state.controlMode, false);
  assert.deepEqual(denied.enabled, [true]);
  assert.deepEqual(setup(false).enabled, []);
  for (const mode of ['pair', 'unattended']) {
    const trusted = setup(true, mode); trusted.input.selectControl(true); trusted.grant();
    assert.equal(trusted.input.state.ready, false);
    assert.deepEqual(trusted.enabled, []);
  }
});

test('canvas viewport configuration is emitted with remote dimensions and reset view is explicit', () => {
  const h = setup();
  h.grant();
  h.input.setViewport(400, 300, 1920, 1080);
  const configure = h.events.at(-1);
  assert.deepEqual(configure, {
    kind: 'configure', viewportWidth: 400, viewportHeight: 300,
    remoteWidth: 1920, remoteHeight: 1080,
    insets: { left: 0, top: 0, right: 0, bottom: 0 }, padding: 24, enabled: true
  });
  h.input.resetView();
  assert.deepEqual(h.events.at(-1), { kind: 'reset_view' });
});

test('canvas state can expose zoom and drag readiness without changing key/text routing', () => {
  const h = setup();
  h.grant();
  h.input.canvasState({ zoom: 2, resetVisible: true, dragReady: true });
  assert.deepEqual(h.input.state.canvas, { zoom: 2, resetVisible: true, dragReady: true });
  h.input.send({ kind: 'text', text: '仍走文字通道' });
  assert.deepEqual(h.events.at(-1), { kind: 'text', text: '仍走文字通道' });
});
