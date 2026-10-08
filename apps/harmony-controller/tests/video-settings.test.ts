import assert from 'node:assert/strict';
import test from 'node:test';
import { buildScreenRequest, initialScreenState, markInputRequest, projectScreenEvent,
  type Profile, type ScreenEvent } from '../entry/src/main/ets/model/DeskModels.ts';
import { readVideoPreferences, validVideoPreferences, VideoPreferences } from '../entry/src/main/ets/model/VideoModels.ts';

test('video preferences use bounded presets and retain the legacy request when absent', () => {
  const profile: Profile = { id: 'local', name: 'PC', mode: 'direct', target: '127.0.0.1:21120',
    peerId: '123456', peerPublicKey: 'key', peerFingerprint: '', server: '', serverKey: '' };
  const legacy = JSON.parse(buildScreenRequest(profile));
  assert.equal(legacy.videoQuality, undefined);
  const modern = JSON.parse(buildScreenRequest(profile, new VideoPreferences()));
  assert.equal(modern.videoQuality, 'balanced');
  assert.equal(modern.videoFps, 15);
  assert.equal(modern.peerPublicKey, legacy.peerPublicKey);
  assert.throws(() => buildScreenRequest(profile, { quality: 'high', fps: 120 }));
  assert.equal(validVideoPreferences('unknown', 15), false);
  assert.deepEqual(readVideoPreferences('{"quality":"high","fps":30}'), { quality: 'high', fps: 30 });
  assert.deepEqual(readVideoPreferences('broken'), new VideoPreferences());
});

function event(state: string): ScreenEvent {
  return { taskId: 1, state, code: '', message: '', verified: true, authenticated: true, authorized: false };
}

test('video selection waits for an authenticated receipt without changing input permission', () => {
  const connected = event('connected');
  connected.videoSettingsSupported = true;
  const approved = projectScreenEvent(initialScreenState(), connected);
  const waiting = projectScreenEvent(approved, event('video_settings_requested'));
  assert.equal(waiting.videoSettingsPending, true);
  assert.equal(waiting.videoQuality, '');
  const receipt = event('video_settings');
  receipt.videoSettingsSupported = true; receipt.videoQuality = 'high'; receipt.videoFps = 30;
  receipt.videoWidth = 2560; receipt.videoHeight = 1440;
  const unauthenticated = { ...receipt, authenticated: false };
  assert.equal(projectScreenEvent(waiting, unauthenticated).videoSettingsPending, true);
  const applied = projectScreenEvent(waiting, receipt);
  assert.equal(applied.videoSettingsPending, false);
  assert.equal(applied.videoQuality, 'high');
  assert.equal(applied.videoFps, 30);
  assert.equal(applied.width, 2560);
  assert.equal(applied.inputGranted, false);
  const inputPending = markInputRequest(applied, true);
  assert.equal(inputPending.videoQuality, 'high');
  assert.equal(projectScreenEvent(inputPending, event('video_status')).videoFps, 30);
});
