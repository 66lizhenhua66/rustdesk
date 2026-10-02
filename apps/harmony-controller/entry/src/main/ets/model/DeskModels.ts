export interface Profile {
  id: string;
  name: string;
  mode: string;
  target: string;
  server: string;
  serverKey: string;
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
  confirmationCode?: string;
  x?: number;
  y?: number;
  textLength?: number;
  videoWidth?: number;
  videoHeight?: number;
  videoCodec?: string;
  frames?: number;
  bytes?: number;
  renderedFrames?: number;
}

export class ScreenState {
  phase: string = 'idle';
  title: string = '';
  detail: string = '';
  verified: boolean = false;
  approved: boolean = false;
  width: number = 1280;
  height: number = 720;
  frames: number = 0;
  rendered: number = 0;
  bytes: number = 0;
  code: string = '';
}

export function initialScreenState(): ScreenState {
  const state = new ScreenState();
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

function failureDetail(code: string): string {
  switch (code) {
    case 'VIDEO_NOT_ENABLED':
      return '被控端尚未启用屏幕共享。请在被控端开启视频服务后重试。';
    case 'MODE_MISMATCH':
      return '当前入口不支持屏幕查看。请核对被控端模式和地址后重试。';
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
    const state = initialScreenState();
    state.code = event.code;
    return state;
  }
  if (event.state === 'verified' || event.state === 'authenticating') {
    const state = new ScreenState();
    state.phase = 'verifying';
    state.title = '正在验证设备';
    state.detail = '正在核对设备身份并建立加密会话…';
    state.verified = event.verified;
    state.code = event.code;
    return state;
  }
  if (event.state === 'awaiting_approval' || event.code === 'AWAITING_APPROVAL') {
    const state = new ScreenState();
    state.phase = 'awaiting';
    state.title = '等待本机批准';
    state.detail = '请在被控端确认屏幕查看请求。批准前不会显示画面。';
    state.verified = event.verified;
    state.code = event.code;
    return state;
  }
  if (event.state === 'connected') {
    const state = new ScreenState();
    state.code = event.code;
    if (!event.verified || !event.authenticated) {
      state.phase = 'failed';
      state.title = '连接状态异常';
      state.detail = '设备身份或登录未确认。请核对可信资料后重试。';
      return state;
    }
    state.phase = 'approved';
    state.title = '已获准查看';
    state.detail = '本机已批准，正在等待第一帧画面；当前仅可查看。';
    state.verified = true;
    state.approved = true;
    if (event.videoWidth !== undefined && event.videoWidth > 0) {
      state.width = event.videoWidth;
    }
    if (event.videoHeight !== undefined && event.videoHeight > 0) {
      state.height = event.videoHeight;
    }
    return state;
  }

  const state = new ScreenState();
  state.phase = previous.phase;
  state.title = previous.title;
  state.detail = previous.detail;
  state.verified = previous.verified;
  state.approved = previous.approved;
  state.width = previous.width;
  state.height = previous.height;
  state.frames = previous.frames;
  state.rendered = previous.rendered;
  state.bytes = previous.bytes;
  state.code = event.code;
  if (!state.approved) {
    return state;
  }
  if (event.state === 'video_status') {
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
    state.detail = '正在查看 Windows 主显示器；未开放键鼠输入。';
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
  return profile.mode === 'direct' && !!profile.peerId && !!profile.peerPublicKey;
}
