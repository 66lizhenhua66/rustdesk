import assert from 'node:assert/strict';
import test from 'node:test';
import {
  canConnectProfile,
  buildScreenRequest,
  connectionPathLabel,
  endedScreenState,
  initialScreenState,
  markInputRequest,
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

test('only an authenticated input-state event grants control and revocation preserves video', () => {
  const input = screenEvent('input_state', 'INPUT_STATE', true, true);
  input.inputSupported = true;
  input.authorized = true;
  assert.equal(projectScreenEvent(initialScreenState(), input).inputGranted, false);
  const approved = projectScreenEvent(initialScreenState(), screenEvent('connected', 'CONNECTED', true, true));
  const legacy = screenEvent('permissions_changed', 'PERMISSIONS_CHANGED', true, true);
  legacy.authorized = true;
  assert.equal(projectScreenEvent(approved, legacy).inputGranted, false);

  const granted = projectScreenEvent(approved, input);
  assert.equal(granted.inputSupported, true);
  assert.equal(granted.inputGranted, true);
  const rendered = screenEvent('video_rendered', 'VIDEO_RENDERED', true, true);
  rendered.renderedFrames = 5;
  const viewing = projectScreenEvent(granted, rendered);
  assert.equal(viewing.inputGranted, true);
  const stats = screenEvent('video_status', 'VIDEO_STATUS', true, true);
  stats.frames = 8;
  assert.equal(projectScreenEvent(viewing, stats).inputGranted, true);

  input.authorized = false;
  const revoked = projectScreenEvent(viewing, input);
  assert.equal(revoked.inputGranted, false);
  assert.equal(revoked.inputSupported, true);
  assert.equal(revoked.phase, 'viewing');
  assert.equal(revoked.rendered, 5);
  input.authorized = true;
  input.inputSupported = false;
  assert.equal(projectScreenEvent(viewing, input).inputGranted, false);
  input.inputSupported = true;
  input.authenticated = false;
  assert.equal(projectScreenEvent(viewing, input).inputGranted, false);
  const ended = projectScreenEvent(viewing, screenEvent('closed', 'DISCONNECTED'));
  assert.equal(ended.inputGranted, false);
  assert.equal(ended.inputSupported, false);
  assert.equal(projectScreenEvent(ended, input).inputGranted, false);
});

test('capability requests wait for input-state confirmation across video callbacks', () => {
  const approved = projectScreenEvent(initialScreenState(), screenEvent('connected', 'CONNECTED', true, true));
  const state = screenEvent('input_state', 'INPUT_STATE', true, true);
  state.inputSupported = true;
  const supported = projectScreenEvent(approved, state);
  const enabling = markInputRequest(supported, true);
  assert.equal(enabling.code, 'INPUT_ENABLING');
  assert.equal(enabling.inputGranted, false);
  assert.equal(supported.code, 'INPUT_STATE');
  const frame = screenEvent('video_rendered', 'VIDEO_RENDERED', true, true);
  frame.renderedFrames = 2;
  const waiting = projectScreenEvent(enabling, frame);
  assert.equal(waiting.code, 'INPUT_ENABLING');
  assert.equal(waiting.inputGranted, false);
  state.authorized = true;
  const enabled = projectScreenEvent(waiting, state);
  assert.equal(enabled.code, 'INPUT_STATE');
  assert.equal(enabled.inputGranted, true);
  const disabling = markInputRequest(enabled, false);
  assert.equal(disabling.inputGranted, true);
  assert.equal(projectScreenEvent(disabling, state).code, 'INPUT_DISABLING');
  const stats = screenEvent('video_status', 'VIDEO_STATUS', true, true);
  stats.frames = 4;
  assert.equal(projectScreenEvent(disabling, stats).code, 'INPUT_DISABLING');
  state.authorized = false;
  const disabled = projectScreenEvent(disabling, state);
  assert.equal(disabled.inputGranted, false);
  assert.equal(projectScreenEvent(disabled, frame).code, 'INPUT_STATE');
  assert.equal(markInputRequest(endedScreenState('ended'), true).phase, 'ended');
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

test('ID and relay connections require a configured service and the same pinned target identity', () => {
  const profile: Profile = {
    id: 'routed', name: 'Desk', mode: 'id', target: '123456', peerId: '123456',
    peerPublicKey: 'peer-key', peerFingerprint: '', server: 'self-host.example:21116',
    serverKey: 'server-key', relayServer: 'self-host.example:21117'
  };
  for (const mode of ['id', 'relay']) {
    profile.mode = mode;
    assert.equal(canConnectProfile(profile), true);
    const request = JSON.parse(buildScreenRequest(profile));
    assert.equal(request.mode, mode);
    assert.equal(request.peerId, '123456');
    assert.equal(request.endpoint, undefined);
    assert.equal(request.server, profile.server);
    assert.equal(request.serverKey, profile.serverKey);
    assert.equal(request.relayServer, profile.relayServer);
    assert.equal(request.minimumKxVersion, 1);
    assert.equal(request.expectedPeer, 'secure_control');
  }
  for (const field of ['server', 'serverKey', 'relayServer', 'peerPublicKey'] as const) {
    assert.equal(canConnectProfile({ ...profile, [field]: '' }), false);
  }
  assert.equal(canConnectProfile({ ...profile, peerId: '654321' }), false);
  assert.equal(canConnectProfile({ ...profile, mode: 'unknown' }), false);
  assert.throws(() => buildScreenRequest({ ...profile, peerId: '654321' }));
});

test('actual connection path survives approval and video updates without granting access', () => {
  const event = screenEvent('transport_selected', 'TRANSPORT_SELECTED');
  event.connectionPath = 'relay';
  const routed = projectScreenEvent(initialScreenState(), event);
  assert.equal(routed.connectionPath, 'relay');
  assert.equal(routed.verified, false);
  assert.equal(routed.approved, false);
  const verified = projectScreenEvent(routed, screenEvent('verified', 'VERIFIED', true));
  const waiting = projectScreenEvent(verified, screenEvent('awaiting_approval', 'AWAITING_APPROVAL', true));
  const approved = projectScreenEvent(waiting, screenEvent('connected', 'CONNECTED', true, true));
  const stats = projectScreenEvent(approved, screenEvent('video_status', 'VIDEO_STATUS', true, true));
  assert.equal(markInputRequest(stats, true).connectionPath, 'relay');
  assert.equal(connectionPathLabel(stats.connectionPath), '中继');
  assert.equal(connectionPathLabel('id_direct'), 'ID 协调直连');
  assert.equal(connectionPathLabel('direct'), 'IP 直连');
  assert.equal(connectionPathLabel(''), '等待连接');
  assert.equal(projectScreenEvent(stats, screenEvent('closed', 'DISCONNECTED')).connectionPath, '');
});

test('coordination failures explain the actual core error without claiming approval', () => {
  for (const [code, explanation] of [
    ['SERVER_IDENTITY_INVALID', /协调服务身份/],
    ['PEER_IDENTITY_MISMATCH', /设备身份/],
    ['PEER_UNAVAILABLE', /未在线/],
    ['RELAY_REJECTED', /中继/],
    ['RELAY_SERVER_MISMATCH', /中继与配置不一致/],
    ['RESOLVE_FAILED', /域名解析/]
  ] as const) {
    const failed = projectScreenEvent(initialScreenState(), screenEvent('failed', code));
    assert.match(failed.detail, explanation);
    assert.equal(failed.approved, false);
    assert.equal(failed.phase, 'failed');
  }
});

test('trusted pairing requires its own verified connected receipt and remains read only', () => {
  const start = initialScreenState('pair');
  const pending = screenEvent('awaiting_approval', 'AWAITING_APPROVAL', true);
  assert.match(projectScreenEvent(start, pending).detail, /登记/);
  const ordinary = screenEvent('connected', 'CONNECTED', true, true);
  assert.equal(projectScreenEvent(start, ordinary).approved, false);
  ordinary.accessMode = 'pair';
  const paired = projectScreenEvent(start, ordinary);
  assert.equal(paired.approved, true);
  assert.equal(paired.accessMode, 'pair');
  assert.match(paired.detail, /登记/);
  const input = screenEvent('input_state', 'INPUT_STATE', true, true);
  input.inputSupported = true;
  input.authorized = true;
  assert.equal(projectScreenEvent(paired, input).inputGranted, false);
});

test('unattended connection cannot be mislabeled by an onsite receipt', () => {
  const start = initialScreenState('unattended');
  const ordinary = screenEvent('connected', 'CONNECTED', true, true);
  assert.equal(projectScreenEvent(start, ordinary).approved, false);
  ordinary.accessMode = 'pair';
  assert.equal(projectScreenEvent(start, ordinary).approved, false);
  ordinary.accessMode = 'unattended';
  const approved = projectScreenEvent(start, ordinary);
  assert.equal(approved.approved, true);
  assert.match(approved.detail, /无人值守/);
});
