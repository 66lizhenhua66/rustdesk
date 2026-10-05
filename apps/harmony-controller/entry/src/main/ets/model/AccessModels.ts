export interface TrustedProfile {
  mode: string;
  target: string;
  peerId?: string;
  peerPublicKey?: string;
  peerFingerprint: string;
  server: string;
  serverKey: string;
  relayServer?: string;
}

export interface SigningIdentity {
  seed: string;
  publicKey: string;
}

export interface AccessGrant {
  credentialId: string;
  credentialSecret: string;
  expiresAt: number;
}

export interface StoredAccess extends AccessGrant {
  targetId: string;
  targetPublicKey: string;
  scope: string;
}

export interface AccessStatus {
  state: string;
  expiresAt: number;
}

const KEY32 = /^[A-Za-z0-9+/]{43}=$/;
const ID16 = /^[A-Za-z0-9+/]{22}==$/;

export function parseIdentity(json: string): SigningIdentity {
  const value = JSON.parse(json) as SigningIdentity;
  if (!KEY32.test(value.seed) || !KEY32.test(value.publicKey)) {
    throw new Error('Invalid controller identity');
  }
  return value;
}

export function parsePairingGrant(event: { state: string; accessMode?: string; credentialId?: string;
  credentialSecret?: string; expiresAt?: number }): AccessGrant {
  if (event.state !== 'access_paired' ||
    !ID16.test(event.credentialId ?? '') || !KEY32.test(event.credentialSecret ?? '') ||
    !Number.isSafeInteger(event.expiresAt ?? 0) || (event.expiresAt ?? 0) <= Date.now()) {
    throw new Error('Invalid pairing grant');
  }
  return { credentialId: event.credentialId!, credentialSecret: event.credentialSecret!, expiresAt: event.expiresAt! };
}

export function credentialForTarget(profile: TrustedProfile, grant: AccessGrant): StoredAccess {
  if (!profile.peerId || !profile.peerPublicKey) { throw new Error('Missing pinned target'); }
  return { targetId: profile.peerId, targetPublicKey: profile.peerPublicKey,
    credentialId: grant.credentialId, credentialSecret: grant.credentialSecret,
    expiresAt: grant.expiresAt, scope: 'windows_primary_view' };
}

export function accessStatus(profile: TrustedProfile, credential?: StoredAccess): AccessStatus {
  if (!credential) { return { state: 'missing', expiresAt: 0 }; }
  if (credential.targetId !== profile.peerId || credential.targetPublicKey !== profile.peerPublicKey ||
    credential.scope !== 'windows_primary_view' || !ID16.test(credential.credentialId) ||
    !KEY32.test(credential.credentialSecret) || !Number.isSafeInteger(credential.expiresAt)) {
    return { state: 'unavailable', expiresAt: 0 };
  }
  return { state: credential.expiresAt <= Date.now() ? 'expired' : 'registered',
    expiresAt: credential.expiresAt };
}

export function secretRequest(mode: string, seed: string, credential?: StoredAccess,
  profile?: TrustedProfile): string {
  if (!KEY32.test(seed)) { throw new Error('Invalid signing seed'); }
  if (mode === 'pair') {
    return JSON.stringify({ mode: 'pair', seed, credentialId: '', credentialSecret: '' });
  }
  if (mode !== 'unattended' || !credential || !profile ||
    credential.targetId !== profile.peerId || credential.targetPublicKey !== profile.peerPublicKey ||
    credential.scope !== 'windows_primary_view') { throw new Error('Credential target mismatch'); }
  if (credential.expiresAt <= Date.now()) { throw new Error('Credential expired'); }
  if (!ID16.test(credential.credentialId) || !KEY32.test(credential.credentialSecret)) {
    throw new Error('Invalid credential');
  }
  return JSON.stringify({ mode, seed, credentialId: credential.credentialId,
    credentialSecret: credential.credentialSecret });
}

export function buildTrustedScreenRequest(profile: TrustedProfile): string {
  if (!profile.peerId || !profile.peerPublicKey || !profile.target) { throw new Error('Missing trusted target'); }
  const common = { peerId: profile.peerId, peerPublicKey: profile.peerPublicKey,
    peerFingerprint: profile.peerFingerprint || null, minimumKxVersion: 1, expectedPeer: 'secure_video' };
  if (profile.mode === 'direct') { return JSON.stringify({ endpoint: profile.target, ...common }); }
  if ((profile.mode !== 'id' && profile.mode !== 'relay') || profile.target !== profile.peerId ||
    !profile.server || !profile.serverKey || !profile.relayServer) { throw new Error('Invalid routed target'); }
  return JSON.stringify({ mode: profile.mode, ...common, server: profile.server,
    serverKey: profile.serverKey, relayServer: profile.relayServer });
}
