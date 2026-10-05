# HarmonyOS controller signing seed storage (2026-10-05)

## Recommendation

For the HarmonyOS 6.1 controller app (target API 24, compatible API 22), store the 32-byte Ed25519 signing seed as the `SECRET` of a single app-private Asset Store item. Use a fixed, nonsensitive `ALIAS`, `DEVICE_FIRST_UNLOCKED`, `AuthType.NONE`, and `SyncType.NEVER`; omit `IS_PERSISTENT`. Query the seed only when starting an authenticated session, pass it to the existing native libsodium signing path, and clear temporary ArkTS/native copies as soon as practical. Never put the seed, its encoding, or a private key in preferences, logs, profile export, or an alias. This preserves the existing Rust/libsodium Ed25519 identity and limits storage migration. Asset Store protects storage, but the seed is plaintext in app memory during use; it is not a nonextractable HUKS signing key.

`DEVICE_FIRST_UNLOCKED` means access after the user's first unlock following boot, including later screen locks. It does **not** grant background execution or a way to run before first unlock. Use `DEVICE_UNLOCKED` only if the intended product policy requires every access while the screen is unlocked; it would prevent unattended use while relocked. `AuthType.NONE` avoids interactive UserIAM authentication on each query. Asset Store defaults are already first-unlocked, no authentication, no sync, and delete on uninstall, but setting the first three explicitly makes this security policy auditable. Sources: [Asset API reference](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/js-apis-asset#accessibility), [add guide](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/asset-js-add).

## Verified API and constraints

- ArkTS import: `import { asset } from '@kit.AssetStoreKit';`. The service starts at API 11, so the app's API 22 minimum is sufficient. `asset.AssetMap` is `Map<asset.Tag, boolean | number | Uint8Array>`. `add(attributes): Promise<void>`, `query(query): Promise<Array<AssetMap>>`, and `remove(query): Promise<void>` are available. The installed API declarations agree at `G:/Huawei/DevEco Studio/sdk/default/openharmony/ets/api/@ohos.security.asset.d.ts`; `apps/harmony-controller/build-profile.json5` targets 6.1.1(24) and supports 6.0.2(22). [API reference](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/js-apis-asset).
- `Tag.SECRET` is 1–1024 bytes, and `Tag.ALIAS` is a unique 1–256 byte index. Alias and data labels are **not encrypted**, so use a fixed opaque label such as `controller-signing-seed-v1`, with no private material or device identifier. [Add guide](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/asset-js-add).
- Set `Tag.RETURN_TYPE` to `asset.ReturnType.ALL` to query plaintext. Query by exact alias returns at most one item; read `result[0].get(asset.Tag.SECRET) as Uint8Array`, then validate exactly 32 bytes before handing it to native code. Plaintext query decrypts and is slower than attribute-only query. A missing item throws `asset.ErrorCode.NOT_FOUND` (`24000002`); it is **not** an empty success result. Distinguish missing from `ACCESS_DENIED` (`24000004`), lock-state mismatch (`24000005`), corruption (`24000007`), and service errors. [Query guide](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/asset-js-query), [API errors](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/js-apis-asset#errorcode).
- `asset.remove` with the exact alias deletes one matching item; missing also throws `24000002`. Handle it as already absent only for an idempotent revoke flow. [Remove guide](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/asset-js-remove).
- No special permission is documented for normal app-owned add/query/remove. `ohos.permission.STORE_PERSISTENT_DATA` is required **only when setting** `Tag.IS_PERSISTENT`; do not set it. Persistent-on-uninstall storage is undesirable for this credential. [Add API](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/js-apis-asset#assetadd), [add guide](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/asset-js-add).

Minimal ArkTS shape (illustrative; route errors through the app's existing error model):

```ts
import { asset } from '@kit.AssetStoreKit';
import { util } from '@kit.ArkTS';
import { BusinessError } from '@kit.BasicServicesKit';

const alias = new util.TextEncoder().encodeInto('controller-signing-seed-v1');

async function saveSeed(seed: Uint8Array): Promise<void> {
  if (seed.length !== 32) throw new Error('Invalid signing seed');
  const attrs: asset.AssetMap = new Map();
  attrs.set(asset.Tag.ALIAS, alias);
  attrs.set(asset.Tag.SECRET, seed);
  attrs.set(asset.Tag.ACCESSIBILITY, asset.Accessibility.DEVICE_FIRST_UNLOCKED);
  attrs.set(asset.Tag.AUTH_TYPE, asset.AuthType.NONE);
  attrs.set(asset.Tag.SYNC_TYPE, asset.SyncType.NEVER);
  await asset.add(attrs); // Duplicate alias is 24000003; do not silently overwrite.
}

async function loadSeed(): Promise<Uint8Array | undefined> {
  const query: asset.AssetMap = new Map();
  query.set(asset.Tag.ALIAS, alias);
  query.set(asset.Tag.RETURN_TYPE, asset.ReturnType.ALL);
  try {
    const rows = await asset.query(query);
    const seed = rows[0].get(asset.Tag.SECRET) as Uint8Array;
    if (seed?.length !== 32) throw new Error('Invalid stored signing seed');
    return seed;
  } catch (error) {
    if ((error as BusinessError).code === asset.ErrorCode.NOT_FOUND) return undefined;
    throw error;
  }
}

async function removeSeed(): Promise<void> {
  const query: asset.AssetMap = new Map();
  query.set(asset.Tag.ALIAS, alias);
  await asset.remove(query);
}
```

For first enrollment, generate the seed with a cryptographic RNG in the native signing layer or the official crypto framework, derive the public key through the same libsodium path, store the seed once, and persist only the public key/profile metadata outside Asset Store. Check the outcome of `add` before exposing trust enrollment as complete. On deletion or migration, remove the Asset item and clear in-memory/native session state; explicitly decide whether replacing an existing identity is allowed rather than using `ConflictResolution.OVERWRITE` implicitly.

## HUKS alternative

HUKS exposes Ed25519 (`HUKS_ALG_ED25519`), private-key import (`HUKS_KEY_TYPE_PRIVATE_KEY` and `HUKS_TAG_IMPORT_KEY_TYPE`), key storage levels (`CE` first unlock, `ECE` currently unlocked), and signing sessions. The ArkTS API's `importKeyItem` accepts a plaintext key in `HuksOptions.inData`; its reference requires algorithm, purpose, and key length parameters. Ordinary HUKS use does not list a permission; requesting SE security level requires restricted `ohos.permission.ACCESS_SE_KEY`. Sources: [HUKS API](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/js-apis-huks), [HUKS signing guide](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/huks-signing-signature-verification-arkts).

These declarations do **not** establish that HUKS accepts a libsodium 32-byte Ed25519 seed in raw form, that it produces the same public key/signature bytes for this protocol, or that the emulator implements the desired secure backend. The official signing example retrieved uses RSA; no Ed25519 seed import/sign interoperability example was found. A HUKS move would also require routing the protocol's sign operation from Rust through NAPI/ArkTS or native HUKS and synchronizing session cancellation. Treat this as a separate spike with a deterministic libsodium vector and both emulator and phone tests, not as a drop-in storage replacement.

## Validation to perform with implementation

1. Compile ArkTS against the installed API 24 SDK and the app's API 22 compatibility setting; build both current ABIs/HAP.
2. On the API 22 phone emulator, add, query, and remove a test identity; verify byte-for-byte seed/public-key agreement with native libsodium, duplicate handling, missing-item `24000002`, and no seed in preferences/logs/exports.
3. Reboot and check query before first unlock fails cleanly, then after first unlock succeeds. Lock again and verify `DEVICE_FIRST_UNLOCKED` remains queryable if the app can legally execute; test background lifecycle separately. An emulator without a lock credential cannot establish the protected first-unlock behavior because the official API says it becomes powered-on access in that case.
4. Repeat security and lifecycle checks on an API 24 phone/tablet device. SDK declarations and docs verify API shape, not device-specific secure storage behavior.
