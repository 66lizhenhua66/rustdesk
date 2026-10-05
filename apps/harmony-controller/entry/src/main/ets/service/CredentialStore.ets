import { asset } from '@kit.AssetStoreKit';
import { util } from '@kit.ArkTS';
import { cryptoFramework } from '@kit.CryptoArchitectureKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { Profile } from '../model/DeskModels';
import { SigningIdentity, StoredAccess, parseIdentity, secretRequest, accessStatus } from '../model/AccessModels';

const IDENTITY_ALIAS = 'ord-controller-identity-v1';
const encoder = new util.TextEncoder();
const base64 = new util.Base64Helper();

function alias(value: string): Uint8Array { return encoder.encodeInto(value); }

function isMissing(error: Error): boolean {
  return (error as BusinessError).code === asset.ErrorCode.NOT_FOUND;
}

function targetAlias(profile: Profile): Uint8Array {
  if (!profile.peerId || !profile.peerPublicKey) { throw new Error('Missing pinned target'); }
  const digest = cryptoFramework.createMd('SHA256');
  digest.updateSync({ data: encoder.encodeInto(profile.peerId + '\0' + profile.peerPublicKey) });
  const bytes = digest.digestSync().data;
  let hex = '';
  for (let i = 0; i < bytes.length; i++) { hex += bytes[i].toString(16).padStart(2, '0'); }
  return alias('ord-access-v1-' + hex);
}

function attributes(key: Uint8Array, secret: Uint8Array): asset.AssetMap {
  const attrs: asset.AssetMap = new Map();
  attrs.set(asset.Tag.ALIAS, key);
  attrs.set(asset.Tag.SECRET, secret);
  attrs.set(asset.Tag.ACCESSIBILITY, asset.Accessibility.DEVICE_FIRST_UNLOCKED);
  attrs.set(asset.Tag.AUTH_TYPE, asset.AuthType.NONE);
  attrs.set(asset.Tag.SYNC_TYPE, asset.SyncType.NEVER);
  return attrs;
}

async function query(key: Uint8Array): Promise<asset.AssetMap | undefined> {
  const request: asset.AssetMap = new Map();
  request.set(asset.Tag.ALIAS, key);
  request.set(asset.Tag.RETURN_TYPE, asset.ReturnType.ALL);
  try {
    const rows = await asset.query(request);
    if (rows.length !== 1) { throw new Error('Invalid credential result'); }
    return rows[0];
  } catch (error) {
    if (isMissing(error as Error)) { return undefined; }
    throw new Error('Asset Store query failed');
  }
}

export async function loadIdentity(): Promise<SigningIdentity | undefined> {
  const row = await query(alias(IDENTITY_ALIAS));
  if (!row) { return undefined; }
  const seed = row.get(asset.Tag.SECRET) as Uint8Array;
  const publicKey = row.get(asset.Tag.DATA_LABEL_CRITICAL_1) as Uint8Array;
  if (seed?.length !== 32 || publicKey?.length !== 32) { throw new Error('Invalid stored identity'); }
  try { return { seed: base64.encodeToStringSync(seed), publicKey: base64.encodeToStringSync(publicKey) }; }
  finally { seed.fill(0); }
}

export async function saveIdentity(json: string): Promise<SigningIdentity> {
  const identity = parseIdentity(json);
  const seed = base64.decodeSync(identity.seed);
  const publicKey = base64.decodeSync(identity.publicKey);
  try {
    if (seed.length !== 32 || publicKey.length !== 32) { throw new Error('Invalid identity bytes'); }
    const attrs = attributes(alias(IDENTITY_ALIAS), seed);
    attrs.set(asset.Tag.DATA_LABEL_CRITICAL_1, publicKey);
    await asset.add(attrs);
    return identity;
  } finally { seed.fill(0); }
}

export async function readAccess(profile: Profile): Promise<StoredAccess | undefined> {
  const row = await query(targetAlias(profile));
  if (!row) { return undefined; }
  const bytes = row.get(asset.Tag.SECRET) as Uint8Array;
  if (!bytes || bytes.length > 1024) { throw new Error('Invalid stored credential'); }
  try {
    const stored = JSON.parse(new util.TextDecoder().decodeToString(bytes)) as StoredAccess;
    if (accessStatus(profile, stored).state === 'unavailable') { throw new Error('Invalid stored credential'); }
    return stored;
  } finally { bytes.fill(0); }
}

export async function loadAccess(profile: Profile): Promise<StoredAccess | undefined> {
  const stored = await readAccess(profile);
  if (stored) { secretRequest('unattended', 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=', stored, profile); }
  return stored;
}

export async function saveAccess(profile: Profile, credential: StoredAccess): Promise<void> {
  secretRequest('unattended', 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=', credential, profile);
  const bytes = encoder.encodeInto(JSON.stringify(credential));
  if (bytes.length > 1024) { throw new Error('Credential exceeds Asset Store limit'); }
  try {
    const attrs = attributes(targetAlias(profile), bytes);
    attrs.set(asset.Tag.CONFLICT_RESOLUTION, asset.ConflictResolution.OVERWRITE);
    await asset.add(attrs);
  } finally { bytes.fill(0); }
}

export async function removeAccess(profile: Profile): Promise<void> {
  const request: asset.AssetMap = new Map();
  request.set(asset.Tag.ALIAS, targetAlias(profile));
  try { await asset.remove(request); }
  catch (error) { if (!isMissing(error as Error)) { throw new Error('Asset Store removal failed'); } }
}
