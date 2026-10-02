import assert from 'node:assert/strict';
import test from 'node:test';
import {
  canConnectProfile,
  endedScreenState,
  initialScreenState,
  joinEndpoint,
  projectScreenEvent,
  screenIsLive,
  splitEndpoint,
  type Profile,
  type ScreenEvent
} from '../entry/src/main/ets/model/DeskModels.ts';

function screenEvent(state: string, code: string = '', verified: boolean = false,
  authenticated: boolean = false): ScreenEvent {
  return {
    taskId: 1, state: state, code: code, message: '', verified: verified,
    authenticated: authenticated, authorized: false
  };
}

test('a new screen moves through verification and local approval without exposing video', () => {
  const start = initialScreenState();
  assert.equal(start.phase, 'connecting');
  assert.equal(start.title, '正在连接设备');
  assert.equal(screenIsLive(start), true);

  const verifying = projectScreenEvent(start, screenEvent('verified', 'IDENTITY_VERIFIED', true));
  assert.equal(verifying.phase, 'verifying');
  const waiting = projectScreenEvent(verifying,
    screenEvent('awaiting_approval', 'AWAITING_APPROVAL', true));
  assert.equal(waiting.phase, 'awaiting');
  assert.equal(waiting.approved, false);
  assert.equal(waiting.rendered, 0);
  assert.equal(screenIsLive(waiting), true);
  assert.equal(start.phase, 'connecting');
});

test('connected requires verified identity and login; demo input permission never grants screen access', () => {
  const waiting = projectScreenEvent(initialScreenState(),
    screenEvent('awaiting_approval', 'AWAITING_APPROVAL', true));
  const unverified = screenEvent('connected', 'CONNECTED', false, true);
  unverified.authorized = true;
  assert.equal(projectScreenEvent(waiting, unverified).approved, false);
  const unauthenticated = screenEvent('connected', 'CONNECTED', true, false);
  assert.equal(projectScreenEvent(waiting, unauthenticated).approved, false);

  const approved = projectScreenEvent(waiting,
    screenEvent('connected', 'CONNECTED', true, true));
  assert.equal(approved.phase, 'approved');
  assert.equal(approved.approved, true);
  assert.equal(approved.verified, true);
  assert.equal(screenIsLive(approved), true);
  const permission = screenEvent('permissions_changed', 'PERMISSIONS_CHANGED', true, true);
  permission.authorized = true;
  assert.equal(projectScreenEvent(approved, permission).approved, true);
});

test('statistics and rendered frames become visible only after approval', () => {
  const waiting = initialScreenState();
  const status = screenEvent('video_status', 'VIDEO_STATUS', true, true);
  status.frames = 12;
  status.bytes = 4096;
  const rendered = screenEvent('video_rendered', 'VIDEO_RENDERED', true, true);
  rendered.renderedFrames = 3;
  assert.equal(projectScreenEvent(waiting, status).frames, 0);
  assert.equal(projectScreenEvent(waiting, rendered).rendered, 0);

  const connected = screenEvent('connected', 'CONNECTED', true, true);
  connected.videoWidth = 1920;
  connected.videoHeight = 1080;
  const approved = projectScreenEvent(waiting, connected);
  assert.equal(approved.width, 1920);
  assert.equal(approved.height, 1080);
  const streaming = projectScreenEvent(approved, status);
  assert.equal(streaming.frames, 12);
  assert.equal(streaming.bytes, 4096);
  assert.equal(streaming.phase, 'approved');
  const viewing = projectScreenEvent(streaming, rendered);
  assert.equal(viewing.phase, 'viewing');
  assert.equal(viewing.rendered, 3);
  assert.equal(streaming.rendered, 0);
});

test('terminal events clear approval and statistics and explain the next action', () => {
  const approved = projectScreenEvent(initialScreenState(),
    screenEvent('connected', 'CONNECTED', true, true));
  const status = screenEvent('video_status', 'VIDEO_STATUS', true, true);
  status.frames = 7;
  status.bytes = 512;
  const rendered = screenEvent('video_rendered', 'VIDEO_RENDERED', true, true);
  rendered.renderedFrames = 2;
  const viewing = projectScreenEvent(projectScreenEvent(approved, status), rendered);
  const failures: string[] = ['VIDEO_NOT_ENABLED', 'MODE_MISMATCH', 'IDENTITY_INVALID', 'TIMEOUT'];
  for (const code of failures) {
    const failed = projectScreenEvent(viewing, screenEvent('failed', code));
    assert.equal(failed.phase, 'failed');
    assert.equal(failed.approved, false);
    assert.equal(failed.rendered, 0);
    assert.equal(failed.frames, 0);
    assert.equal(failed.bytes, 0);
    assert.notEqual(failed.title, '');
    assert.match(failed.detail, /请|稍后/);
    assert.equal(screenIsLive(failed), false);
  }
  const closed = projectScreenEvent(viewing, screenEvent('closed', 'DISCONNECTED'));
  assert.equal(closed.phase, 'ended');
  assert.equal(closed.approved, false);
  assert.match(closed.detail, /重新批准/);
  const disconnected = projectScreenEvent(viewing, screenEvent('failed', 'DISCONNECTED'));
  assert.equal(disconnected.phase, 'ended');
  assert.equal(disconnected.code, 'DISCONNECTED');
  assert.equal(disconnected.verified, false);
  assert.equal(disconnected.approved, false);
  assert.equal(disconnected.frames, 0);
  assert.equal(disconnected.rendered, 0);
  assert.equal(disconnected.bytes, 0);
  assert.match(disconnected.detail, /已断开.*重新批准/);
  assert.equal(projectScreenEvent(viewing, screenEvent('cancelled', 'CANCELLED')).phase, 'ended');
  const ended = endedScreenState('已断开连接');
  assert.equal(ended.code, 'CANCELLED');
  assert.equal(ended.phase, 'ended');
  assert.equal(ended.detail, '已断开连接');
});

test('endpoint fields round trip IPv4 and IPv6; readiness requires direct trusted peer', () => {
  const ipv4 = splitEndpoint('192.0.2.10:21121');
  assert.equal(ipv4.address, '192.0.2.10');
  assert.equal(ipv4.port, '21121');
  const bracketedIpv6 = splitEndpoint('[2001:db8::1]:21122');
  assert.equal(bracketedIpv6.address, '2001:db8::1');
  assert.equal(bracketedIpv6.port, '21122');
  const bareIpv6 = splitEndpoint('2001:db8::1');
  assert.equal(bareIpv6.address, '2001:db8::1');
  assert.equal(bareIpv6.port, '21120');
  assert.equal(joinEndpoint('2001:db8::1', '21122'), '[2001:db8::1]:21122');
  assert.equal(joinEndpoint('192.0.2.10', '21121'), '192.0.2.10:21121');

  const profile: Profile = {
    id: '1', name: 'Desk', mode: 'direct', target: '192.0.2.10', server: '',
    serverKey: '', peerFingerprint: '', peerId: '123456', peerPublicKey: 'trusted-key'
  };
  assert.equal(canConnectProfile(profile), true);
  profile.peerPublicKey = '';
  assert.equal(canConnectProfile(profile), false);
  profile.peerPublicKey = 'trusted-key';
  profile.mode = 'relay';
  assert.equal(canConnectProfile(profile), false);
});
