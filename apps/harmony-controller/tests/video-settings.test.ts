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
  assert.equal(modern.videoResolutionMode, 'preserve');
  assert.equal(modern.videoResolutionWidth, 1920);
  assert.equal(modern.peerPublicKey, legacy.peerPublicKey);
  assert.throws(() => buildScreenRequest(profile, { quality: 'high', fps: 120,
    resolutionMode: 'preserve', resolutionWidth: 0, resolutionHeight: 0 }));
  assert.equal(validVideoPreferences('unknown', 15), false);
  assert.deepEqual({ ...readVideoPreferences('{"quality":"high","fps":30}') },
    { quality: 'high', fps: 30, resolutionMode: 'preserve', resolutionWidth: 1920, resolutionHeight: 1080 });
  assert.equal(readVideoPreferences('{"quality":"high","fps":30,"resolutionMode":"sync","resolutionWidth":1280,"resolutionHeight":720}').resolutionMode, 'preserve');
  assert.deepEqual(readVideoPreferences('broken'), new VideoPreferences());
});

test('a failed desktop change keeps the confirmed settings and resolves pending without granting input', () => {
  const connected = event('connected'); connected.videoSettingsSupported = true;
  const approved = projectScreenEvent(initialScreenState(), connected);
  const receipt: ScreenEvent = { ...event('video_settings'), videoSettingsSupported: true,
    videoSettingsVersion: 2, videoQuality: 'balanced', videoFps: 15,
    videoWidth: 1920, videoHeight: 1080, videoResolutionMode: 'preserve',
    videoResolutionWidth: 1920, videoResolutionHeight: 1080,
    desktopWidth: 3840, desktopHeight: 2160, originalWidth: 3840, originalHeight: 2160,
    resolutionSyncSupported: true, supportedResolutions: [{ width: 1920, height: 1080 }] };
  const confirmed = projectScreenEvent(approved, receipt);
  const waiting = projectScreenEvent(confirmed, event('video_settings_requested'));
  const failed = projectScreenEvent(waiting, { ...receipt, videoSettingsError: 'DISPLAY_SWITCH_FAILED' });
  assert.equal(failed.videoSettingsPending, false);
  assert.equal(failed.videoResolutionMode, 'preserve');
  assert.equal(failed.desktopWidth, 3840);
  assert.equal(failed.inputGranted, false);
  assert.equal(failed.videoSettingsError, 'DISPLAY_SWITCH_FAILED');
  assert.equal(markInputRequest(failed, true).originalWidth, 3840);
  assert.equal(projectScreenEvent(failed, event('video_status')).supportedResolutions.length, 1);
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
