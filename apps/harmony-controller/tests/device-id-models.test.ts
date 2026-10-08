import assert from 'node:assert/strict';
import test from 'node:test';
import type { Profile } from '../entry/src/main/ets/model/DeskModels.ts';
import { connectionCandidates, selectedConnectionProfile } from '../entry/src/main/ets/model/DeviceIdModels.ts';

function profile(id: string, mode: string, peerId: string, server: string = 'id.example:21116'): Profile {
  return { id, name: id, mode, target: mode === 'direct' ? '192.0.2.10:21120' : peerId,
    server, serverKey: 'server-key', relayServer: 'relay.example:21117',
    peerId, peerPublicKey: `peer-key-${id}`, peerFingerprint: '' };
}

test('connection entry filters saved devices by mode and exact device ID', () => {
  const devices = [profile('direct', 'direct', '123456'), profile('id', 'id', '123456'),
    profile('relay', 'relay', '123456'), profile('other', 'id', '654321')];
  assert.deepEqual(connectionCandidates(devices, 'id', ' 123456 '), [devices[1]]);
  assert.equal(selectedConnectionProfile(devices, 'id', '123456', ''), devices[1]);
  assert.deepEqual(connectionCandidates(devices, 'relay', '123456'), [devices[2]]);
  assert.deepEqual(connectionCandidates(devices, 'direct', ''), [devices[0]]);
});

test('unknown device IDs never reuse the previously selected trusted identity', () => {
  const saved = profile('saved', 'id', '123456');
  assert.equal(selectedConnectionProfile([saved], 'id', '654321', saved.id), undefined);
  assert.equal(selectedConnectionProfile([saved], 'relay', '123456', saved.id), undefined);
  assert.equal(selectedConnectionProfile([saved], 'id', '', ''), undefined);
});

test('the same device ID on multiple services requires an explicit saved profile selection', () => {
  const devices = [profile('home', 'id', '123456', 'home.example:21116'),
    profile('work', 'id', '123456', 'work.example:21116')];
  assert.equal(selectedConnectionProfile(devices, 'id', '123456', ''), undefined);
  assert.equal(selectedConnectionProfile(devices, 'id', '123456', 'work'), devices[1]);
  assert.equal(selectedConnectionProfile(devices, 'id', '123456', 'home')?.peerPublicKey, 'peer-key-home');
});
