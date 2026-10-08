import type { Profile } from './DeskModels';

export function connectionCandidates(profiles: Profile[], mode: string, peerId: string): Profile[] {
  const target = peerId.trim();
  return profiles.filter((profile: Profile) => profile.mode === mode &&
    (mode === 'direct' || target.length === 0 || profile.peerId === target));
}

export function selectedConnectionProfile(profiles: Profile[], mode: string, peerId: string,
  selectedId: string): Profile | undefined {
  const candidates = connectionCandidates(profiles, mode, peerId);
  const selected = candidates.find((profile: Profile) => profile.id === selectedId);
  if (selected) { return selected; }
  if (candidates.length === 1 && (mode === 'direct' || peerId.trim().length > 0)) { return candidates[0]; }
  return undefined;
}
