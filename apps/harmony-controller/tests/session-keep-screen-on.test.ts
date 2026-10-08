import assert from 'node:assert/strict';
import test from 'node:test';
import { SessionKeepScreenOn } from '../entry/src/main/ets/service/SessionKeepScreenOn.ts';
import type { KeepScreenOnSetter } from '../entry/src/main/ets/service/SessionKeepScreenOn.ts';

const settle = () => new Promise<void>((resolve) => setImmediate(resolve));

test('active session keeps the screen on once and releases it when hidden or stopped', async () => {
  const calls: boolean[] = [];
  const session = new SessionKeepScreenOn(async () => async (enabled: boolean) => { calls.push(enabled); });
  session.setEnabled(true);
  await settle();
  for (let frame = 0; frame < 30; frame++) { session.setEnabled(true); }
  await settle();
  assert.deepEqual(calls, [true]);
  session.setEnabled(false);
  await settle();
  session.setEnabled(true);
  await settle();
  session.stop();
  session.setEnabled(true);
  await settle();
  assert.deepEqual(calls, [true, false, true, false]);
});

test('cancelling before the window arrives never enables screen-on', async () => {
  for (const dispose of [false, true]) {
    const windowReady = Promise.withResolvers<KeepScreenOnSetter>();
    const calls: boolean[] = [];
    const session = new SessionKeepScreenOn(() => windowReady.promise);
    session.setEnabled(true);
    if (dispose) { session.stop(); } else { session.setEnabled(false); }
    windowReady.resolve(async (enabled: boolean) => { calls.push(enabled); });
    await settle();
    assert.deepEqual(calls, []);
  }
});

test('stopping during an outstanding native enable releases it after completion', async () => {
  const enabled = Promise.withResolvers<void>();
  const calls: boolean[] = [];
  const session = new SessionKeepScreenOn(async () => async (value: boolean) => {
    calls.push(value);
    if (value) { await enabled.promise; }
  });
  session.setEnabled(true);
  await settle();
  session.stop();
  enabled.resolve();
  await settle();
  assert.deepEqual(calls, [true, false]);
});
