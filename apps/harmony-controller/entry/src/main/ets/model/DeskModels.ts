import type { VideoPreferences } from './VideoModels';

export interface Profile {
  id: string;
  name: string;
  mode: string;
  target: string;
  server: string;
  serverKey: string;
  relayServer?: string;
  peerFingerprint: string;
  peerId?: string;
  peerPublicKey?: string;
}

export interface ScreenEvent {
  taskId: number;
  state: string;
  code: string;
  message: string;
  verified: boolean;
  authenticated: boolean;
  authorized: boolean;
  inputSupported?: boolean;
  confirmationCode?: string;
  x?: number;
  y?: number;
  textLength?: number;
  videoWidth?: number;
  videoHeight?: number;
  videoCodec?: string;
  videoQuality?: string;
  videoFps?: number;
  videoSettingsSupported?: boolean;
  videoRequestId?: number;
  frames?: number;
  bytes?: number;
  renderedFrames?: number;
  connectionPath?: string;
  accessMode?: string;
  credentialId?: string;
  credentialSecret?: string;
  expiresAt?: number;
}

export class ScreenState {
  phase: string = 'idle';
  title: string = '';
  detail: string = '';
  verified: boolean = false;
  approved: boolean = false;
  inputSupported: boolean = false;
  inputGranted: boolean = false;
  width: number = 1280;
  height: number = 720;
  videoQuality: string = '';
  videoFps: number = 0;
  videoSettingsSupported: boolean = false;
  videoSettingsPending: boolean = false;
  frames: number = 0;
  rendered: number = 0;
  bytes: number = 0;
  code: string = '';
  connectionPath: string = '';
  requestedAccessMode: string = '';
  accessMode: string = '';
}

export function initialScreenState(requestedAccessMode: string = ''): ScreenState {
  const state = new ScreenState();
  state.requestedAccessMode = requestedAccessMode;
  state.phase = 'connecting';
  state.title = '正在连接设备';
  state.detail = '正在准备安全连接…';
  return state;
}

export function endedScreenState(detail: string, code: string = 'CANCELLED'): ScreenState {
  const state = new ScreenState();
  state.phase = 'ended';
  state.title = '连接已结束';
  state.detail = detail;
  state.code = code;
  return state;
}

export function screenIsLive(state: ScreenState): boolean {
  return state.phase === 'connecting' || state.phase === 'verifying' ||
    state.phase === 'awaiting' || state.phase === 'approved' || state.phase === 'viewing';
}

export function markInputRequest(previous: ScreenState, enabled: boolean): ScreenState {
  if (!previous.approved || !screenIsLive(previous)) { return previous; }
  const state = new ScreenState();
  state.phase = previous.phase;
  state.connectionPath = previous.connectionPath;
  state.requestedAccessMode = previous.requestedAccessMode;
  state.accessMode = previous.accessMode;
  state.title = previous.title;
  state.detail = previous.detail;
  state.verified = previous.verified;
  state.approved = previous.approved;
  state.inputSupported = previous.inputSupported;
  state.inputGranted = previous.inputGranted;
  state.width = previous.width;
  state.height = previous.height;
  state.videoQuality = previous.videoQuality;
  state.videoFps = previous.videoFps;
  state.videoSettingsSupported = previous.videoSettingsSupported;
  state.videoSettingsPending = previous.videoSettingsPending;
  state.frames = previous.frames;
  state.rendered = previous.rendered;
  state.bytes = previous.bytes;
  state.code = enabled ? 'INPUT_ENABLING' : 'INPUT_DISABLING';
  return state;
}

function failureDetail(code: string): string {
  if (code === 'INPUT_RELEASE_FAILED') {
    return '输入许可已失效，但 Windows 按键释放失败。请在本机检查按键状态，并重启被控服务后重试。';
  }
  switch (code) {
    case 'VIDEO_NOT_ENABLED':
      return '被控端尚未启用屏幕共享。请在被控端开启视频服务后重试。';
    case 'UNSUPPORTED_VIDEO_SETTINGS':
      return '被控端不支持画质设置，请更新 Windows 被控端后重连。';
    case 'VIDEO_SETTINGS_TIMEOUT':
    case 'INVALID_VIDEO_SETTINGS':
    case 'INVALID_VIDEO_STATE':
      return '画质切换未得到有效确认，连接已停止。请检查被控端并重新连接。';
    case 'MODE_MISMATCH':
      return '当前入口不支持此控制协议。请更新被控端，或在连接诊断中使用原只读入口。';
    case 'IDENTITY_REQUIRED':
    case 'IDENTITY_INVALID':
    case 'INVALID_SIGNATURE':
    case 'SIGNATURE_INVALID':
    case 'PEER_ID_MISMATCH':
    case 'INVALID_PEER_ID':
    case 'TRUST_FAILED':
    case 'FINGERPRINT_MISMATCH':
    case 'KEY_MISMATCH':
    case 'MISSING_PEER_KEY':
    case 'MISSING_KEY':
    case 'TRUST_MATERIAL_MISSING':
      return '设备身份与保存的可信资料不一致或资料缺失。请核对设备 ID 和公钥后重试。';
    case 'TIMEOUT':
      return '等待连接或现场批准超时。请检查网络和被控端请求后重试。';
    case 'LOGIN_REJECTED':
    case 'PASSWORD_REJECTED':
      return '被控端未接受连接。请在被控端确认请求后重试。';
    case 'TRANSPORT_FAILED':
      return '无法连接被控端。请检查目标地址和网络后重试。';
    case 'SERVER_TRUST_FAILED':
    case 'SERVER_KEY_MISMATCH':
    case 'INVALID_SERVER_SIGNATURE':
    case 'SERVER_IDENTITY_INVALID':
      return '协调服务身份校验失败。请核对自建服务地址和验证公钥。';
    case 'PEER_OFFLINE':
    case 'ID_NOT_EXIST':
    case 'PEER_UNAVAILABLE':
      return '目标设备未在线或设备 ID 不存在。请检查 Windows 服务注册状态。';
    case 'RELAY_FAILED':
    case 'RELAY_REFUSED':
    case 'RELAY_REJECTED':
      return '无法通过配置的中继连接。请检查中继地址和服务状态。';
    case 'PEER_IDENTITY_INVALID':
    case 'PEER_IDENTITY_MISMATCH':
      return '服务登记的设备身份与保存资料不一致。请现场核对设备 ID 和公钥。';
    case 'RELAY_MISMATCH':
    case 'RELAY_SERVER_MISMATCH':
      return '服务返回的中继与配置不一致。请核对自建服务的中继设置。';
    case 'RENDEZVOUS_PROTOCOL_FAILED':
      return '协调服务返回了无法处理的连接信息。请核对服务版本和配置。';
    case 'RESOLVE_FAILED':
    case 'RESOLVER_BUSY':
      return '服务域名解析失败或仍在处理中。请检查地址和网络后重试。';
    default:
      return '无法建立远程画面（' + code + '）。请检查被控端状态后重试。';
  }
}

export function projectScreenEvent(previous: ScreenState, event: ScreenEvent): ScreenState {
  if (previous.phase === 'ended' || previous.phase === 'failed') {
    return previous;
  }

  if (event.code === 'DISCONNECTED' || event.state === 'closed' || event.state === 'cancelled') {
    const detail = event.state === 'cancelled'
      ? '连接已取消。请重新连接以发起新的批准请求。'
      : '远程画面已断开。请检查被控端后重新连接，届时需要重新批准查看。';
    return endedScreenState(detail, event.code);
  }
  if (event.state === 'failed' || event.state === 'blocked') {
    const state = new ScreenState();
    state.phase = 'failed';
    state.title = '连接失败';
    state.detail = failureDetail(event.code);
    state.code = event.code;
    return state;
  }
  if (event.state === 'connecting') {
    const state = initialScreenState(previous.requestedAccessMode);
    state.code = event.code;
    return state;
  }
  if (event.state === 'transport_selected' && !previous.approved) {
    const state = initialScreenState(previous.requestedAccessMode);
    const path = event.connectionPath ?? '';
    state.connectionPath = path === 'direct' || path === 'id_direct' || path === 'relay' ? path : '';
    state.code = event.code;
    return state;
  }
  if (event.state === 'verified' || event.state === 'authenticating') {
    const state = new ScreenState();
    state.requestedAccessMode = previous.requestedAccessMode;
    state.connectionPath = previous.connectionPath;
    state.phase = 'verifying';
    state.title = '正在验证设备';
    state.detail = '正在核对设备身份并建立加密会话…';
    state.verified = event.verified;
    state.code = event.code;
    return state;
  }
  if (event.state === 'awaiting_approval' || event.code === 'AWAITING_APPROVAL') {
    const state = new ScreenState();
    state.requestedAccessMode = previous.requestedAccessMode;
    state.connectionPath = previous.connectionPath;
    state.phase = 'awaiting';
    state.title = '等待本机批准';
    state.detail = previous.requestedAccessMode === 'pair'
      ? '请在 Windows 本机明确批准并登记只读访问。普通连接批准不会登记设备。'
      : '请在被控端确认屏幕查看请求。批准前不会显示画面。';
    state.verified = event.verified;
    state.code = event.code;
    return state;
  }
  if (event.state === 'connected') {
    const state = new ScreenState();
    state.requestedAccessMode = previous.requestedAccessMode;
    state.connectionPath = previous.connectionPath;
    state.code = event.code;
    if (!event.verified || !event.authenticated ||
      (previous.requestedAccessMode && event.accessMode !== previous.requestedAccessMode)) {
      state.phase = 'failed';
      state.title = '连接状态异常';
      state.detail = '设备身份或登录未确认。请核对可信资料后重试。';
      return state;
    }
    state.phase = 'approved';
    state.accessMode = previous.requestedAccessMode;
    state.title = previous.requestedAccessMode === 'unattended' ? '无人值守只读连接已验证'
      : previous.requestedAccessMode === 'pair' ? '登记已获批准' : '已获准查看';
    state.detail = previous.requestedAccessMode === 'unattended'
      ? '无人值守凭据已验证，正在等待第一帧画面；当前仅可查看。'
      : previous.requestedAccessMode === 'pair'
        ? 'Windows 已批准登记，正在等待第一帧画面；当前仅可查看。'
        : '本机已批准，正在等待第一帧画面；当前仅可查看。';
    state.verified = true;
    state.approved = true;
    state.videoSettingsSupported = event.videoSettingsSupported === true;
    if (event.videoWidth !== undefined && event.videoWidth > 0) {
      state.width = event.videoWidth;
    }
    if (event.videoHeight !== undefined && event.videoHeight > 0) {
      state.height = event.videoHeight;
    }
    return state;
  }

  const state = new ScreenState();
  state.requestedAccessMode = previous.requestedAccessMode;
  state.accessMode = previous.accessMode;
  state.phase = previous.phase;
  state.connectionPath = previous.connectionPath;
  state.title = previous.title;
  state.detail = previous.detail;
  state.verified = previous.verified;
  state.approved = previous.approved;
  state.inputSupported = previous.inputSupported;
  state.inputGranted = previous.inputGranted;
  state.width = previous.width;
  state.height = previous.height;
  state.videoQuality = previous.videoQuality;
  state.videoFps = previous.videoFps;
  state.videoSettingsSupported = previous.videoSettingsSupported;
  state.videoSettingsPending = previous.videoSettingsPending;
  state.frames = previous.frames;
  state.rendered = previous.rendered;
  state.bytes = previous.bytes;
  const inputCode = previous.code === 'INPUT_STATE' || previous.code === 'INPUT_ENABLING' || previous.code === 'INPUT_DISABLING';
  state.code = inputCode && (event.state === 'video_status' || event.state === 'video_rendered') ? previous.code : event.code;
  if (!state.approved) {
    return state;
  }
  if (event.state === 'video_settings_requested' && state.videoSettingsSupported && !state.accessMode) {
    state.code = previous.code;
    state.videoSettingsPending = true;
  } else if (event.state === 'video_settings' && event.verified && event.authenticated &&
    event.videoSettingsSupported === true && !state.accessMode &&
    (event.videoQuality === 'low' || event.videoQuality === 'balanced' || event.videoQuality === 'high') &&
    (event.videoFps === 10 || event.videoFps === 15 || event.videoFps === 30) &&
    event.videoWidth !== undefined && event.videoWidth >= 2 && event.videoWidth <= 2560 &&
    event.videoHeight !== undefined && event.videoHeight >= 2 && event.videoHeight <= 1440) {
    state.code = previous.code;
    state.videoQuality = event.videoQuality;
    state.videoFps = event.videoFps;
    state.videoSettingsSupported = true;
    state.videoSettingsPending = false;
    state.width = event.videoWidth;
    state.height = event.videoHeight;
  } else if (event.state === 'input_state') {
    state.inputSupported = !state.accessMode && event.verified && event.authenticated && event.inputSupported === true;
    state.inputGranted = state.inputSupported && event.authorized === true;
    if (previous.code === 'INPUT_DISABLING' && state.inputGranted) { state.code = 'INPUT_DISABLING'; }
  } else if (event.state === 'video_status') {
    if (event.frames !== undefined && event.frames >= 0) {
      state.frames = event.frames;
    }
    if (event.bytes !== undefined && event.bytes >= 0) {
      state.bytes = event.bytes;
    }
  } else if (event.state === 'video_rendered' &&
    event.renderedFrames !== undefined && event.renderedFrames > 0) {
    state.rendered = event.renderedFrames;
    state.phase = 'viewing';
    state.title = '正在查看远程桌面';
    state.detail = state.accessMode ? '正在只读查看 Windows 主显示器。'
      : '正在查看 Windows 主显示器；可在控制端开关已支持的键鼠能力。';
  }
  return state;
}

export class EndpointParts {
  address: string = '';
  port: string = '21120';
}

export function splitEndpoint(target: string): EndpointParts {
  const parts = new EndpointParts();
  const value = target.trim();
  if (value.startsWith('[')) {
    const closing = value.indexOf(']');
    if (closing > 0 && (closing === value.length - 1 || value.charAt(closing + 1) === ':')) {
      parts.address = value.substring(1, closing);
      if (closing + 1 < value.length) {
        parts.port = value.substring(closing + 2);
      }
      return parts;
    }
  }
  const firstColon = value.indexOf(':');
  if (firstColon >= 0 && firstColon === value.lastIndexOf(':')) {
    parts.address = value.substring(0, firstColon);
    parts.port = value.substring(firstColon + 1);
  } else {
    parts.address = value;
  }
  return parts;
}

export function joinEndpoint(address: string, port: string): string {
  const host = address.trim();
  const servicePort = port.trim();
  if (host.indexOf(':') >= 0 && !host.startsWith('[')) {
    return '[' + host + ']:' + servicePort;
  }
  return host + ':' + servicePort;
}

export function canConnectProfile(profile: Profile): boolean {
  if (!profile.peerId || !profile.peerPublicKey || !profile.target) { return false; }
  if (profile.mode === 'direct') { return true; }
  return (profile.mode === 'id' || profile.mode === 'relay') && profile.target === profile.peerId &&
    !!profile.server && !!profile.serverKey && !!profile.relayServer;
}

interface DirectScreenRequest {
  endpoint: string;
  peerId: string;
  peerPublicKey: string;
  peerFingerprint: string | null;
  minimumKxVersion: number;
  expectedPeer: string;
  videoQuality?: string;
  videoFps?: number;
}

interface RoutedScreenRequest {
  mode: string;
  peerId: string;
  peerPublicKey: string;
  peerFingerprint: string | null;
  minimumKxVersion: number;
  expectedPeer: string;
  server: string;
  serverKey: string;
  relayServer: string;
  videoQuality?: string;
  videoFps?: number;
}

export function buildScreenRequest(profile: Profile, video?: VideoPreferences): string {
  if (!canConnectProfile(profile)) { throw new Error('连接资料或可信身份不完整'); }
  if (video && (!(video.quality === 'low' || video.quality === 'balanced' || video.quality === 'high') ||
    !(video.fps === 10 || video.fps === 15 || video.fps === 30))) { throw new Error('画质或帧率无效'); }
  if (profile.mode === 'direct') {
    const request: DirectScreenRequest = {
      endpoint: profile.target, peerId: profile.peerId ?? '', peerPublicKey: profile.peerPublicKey ?? '',
      peerFingerprint: profile.peerFingerprint || null, minimumKxVersion: 1, expectedPeer: 'secure_control'
    };
    if (video) { request.videoQuality = video.quality; request.videoFps = video.fps; }
    return JSON.stringify(request);
  }
  const request: RoutedScreenRequest = {
    mode: profile.mode, peerId: profile.peerId ?? '', peerPublicKey: profile.peerPublicKey ?? '',
    peerFingerprint: profile.peerFingerprint || null, minimumKxVersion: 1, expectedPeer: 'secure_control',
    server: profile.server, serverKey: profile.serverKey, relayServer: profile.relayServer ?? ''
  };
  if (video) { request.videoQuality = video.quality; request.videoFps = video.fps; }
  return JSON.stringify(request);
}

export function connectionPathLabel(path: string): string {
  if (path === 'direct') { return 'IP 直连'; }
  if (path === 'id_direct') { return 'ID 协调直连'; }
  if (path === 'relay') { return '中继'; }
  return '等待连接';
}
