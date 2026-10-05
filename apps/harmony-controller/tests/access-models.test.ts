import assert from 'node:assert/strict';
import test from 'node:test';
import { accessStatus, buildTrustedScreenRequest, credentialForTarget, parseIdentity,
  parsePairingGrant, secretRequest } from '../entry/src/main/ets/model/AccessModels.ts';
import type { Profile } from '../entry/src/main/ets/model/DeskModels.ts';

const profile: Profile = {
  id: 'device', name: 'Desk', mode: 'id', target: '123456', peerId: '123456',
  peerPublicKey: 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=', peerFingerprint: '',
  server: 'example.test:21116', serverKey: 'server-key', relayServer: 'example.test:21117'
};
const seed = 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=';
const credentialId = 'AAAAAAAAAAAAAAAAAAAAAA==';
const credentialSecret = 'BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBA=';

test('trusted request retains pinned identity and asks only for secure video', () => {
  const request = JSON.parse(buildTrustedScreenRequest(profile));
  assert.equal(request.expectedPeer, 'secure_video');
  assert.equal(request.peerId, profile.peerId);
  assert.equal(request.peerPublicKey, profile.peerPublicKey);
  assert.equal(request.mode, 'id');
});

test('pairing secret has no credential until server grants one', () => {
  assert.deepEqual(JSON.parse(secretRequest('pair', seed)), {
    mode: 'pair', seed, credentialId: '', credentialSecret: ''
  });
  assert.throws(() => secretRequest('unattended', seed), /[Cc]redential/);
});

test('grant and stored credential are bound to the pinned target', () => {
  const grant = parsePairingGrant({ state: 'access_paired',
    credentialId, credentialSecret, expiresAt: Date.now() + 100000 });
  const bound = credentialForTarget(profile, grant);
  assert.equal(bound.targetId, '123456');
  assert.equal(bound.targetPublicKey, profile.peerPublicKey);
  assert.equal(bound.scope, 'windows_primary_view');
  assert.throws(() => secretRequest('unattended', seed, bound, { ...profile, peerId: '654321' }), /target/);
  assert.equal(JSON.parse(secretRequest('unattended', seed, bound, profile)).credentialId, credentialId);
});

test('malformed identity, grant, and expired credential are rejected', () => {
  assert.throws(() => parseIdentity('{}'), /identity/);
  assert.throws(() => parsePairingGrant({ state: 'access_paired', credentialId,
    credentialSecret, expiresAt: 0 }), /grant/);
  assert.throws(() => secretRequest('unattended', seed, {
    targetId: '123456', targetPublicKey: profile.peerPublicKey, credentialId,
    credentialSecret, expiresAt: Date.now() - 1, scope: 'windows_primary_view'
  }, profile), /expired/);
});

test('device status reflects only a target-bound stored grant and its expiry', () => {
  const credential = credentialForTarget(profile, {
    credentialId, credentialSecret, expiresAt: Date.now() + 100000
  });
  assert.equal(accessStatus(profile, undefined).state, 'missing');
  assert.equal(accessStatus(profile, credential).state, 'registered');
  assert.equal(accessStatus(profile, { ...credential, expiresAt: Date.now() - 1 }).state, 'expired');
  assert.equal(accessStatus({ ...profile, peerId: '654321' }, credential).state, 'unavailable');
});
