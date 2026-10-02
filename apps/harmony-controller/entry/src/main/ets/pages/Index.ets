import controller, { ControllerEvent } from 'libcontroller.so';
import { preferences } from '@kit.ArkData';

interface Profile {
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

interface ValidationResult {
  ok: boolean;
  profile?: Profile;
  code?: string;
  message?: string;
  readyForSession?: boolean;
  reason?: string;
}

const STORE_NAME: string = 'controller_profiles';
const STORE_KEY: string = 'profiles';
let profileCounter: number = 0;

@Entry
@Component
struct Index {
  @State private profiles: Profile[] = [];
  @State private loading: boolean = true;
  @State private saving: boolean = false;
  @State private storeError: string = '';
  @State private formOpen: boolean = false;
  @State private editingId: string = '';
  @State private name: string = '';
  @State private mode: string = 'direct';
  @State private target: string = '';
  @State private server: string = '';
  @State private serverKey: string = '';
  @State private peerFingerprint: string = '';
  @State private peerId: string = '';
  @State private peerPublicKey: string = '';
  @State private formError: string = '';
  @State private selectedId: string = '';
  @State private status: string = '';
  @State private probing: boolean = false;
  @State private activeKind: string = '';
  @State private authPromptOpen: boolean = false;
  @State private sessionPassword: string = '';
  @State private importOpen: boolean = false;
  @State private importJson: string = '';
  @State private importPreview: Profile | undefined = undefined;
  @State private importError: string = '';
  @State private demoConnected: boolean = false;
  @State private demoAuthorized: boolean = false;
  @State private confirmationCode: string = '';
  @State private demoX: number = 0;
  @State private demoY: number = 0;
  @State private demoTextLength: number = 0;
  @State private demoText: string = '';
  @State private demoInputStatus: string = '';
  private store?: preferences.Preferences;
  private currentTask: number = 0;
  private epoch: number = 0;
  private visible: boolean = true;
  private loadId: number = 0;
  private loadActive: boolean = false;
  private padWidth: number = 0;
  private padHeight: number = 0;
  private lastPointerAt: number = 0;

  aboutToAppear(): void {
    this.visible = true;
    this.loadProfiles();
  }

  onPageShow(): void {
    this.visible = true;
    if (this.loading && !this.loadActive) { this.loadProfiles(); }
  }

  onPageHide(): void {
    this.stopPage();
  }

  aboutToDisappear(): void {
    this.stopPage();
  }

  private stopPage(): void {
    if (this.probing) { this.status = '检查已停止；操作未授权。'; }
    this.visible = false;
    this.epoch++;
    this.loadId++;
    this.loadActive = false;
    this.currentTask = 0;
    this.probing = false;
    this.activeKind = '';
    this.demoConnected = false;
    this.demoAuthorized = false;
    this.confirmationCode = '';
    this.demoX = 0;
    this.demoY = 0;
    this.demoTextLength = 0;
    this.demoText = '';
    this.demoInputStatus = '';
    this.lastPointerAt = 0;
    this.authPromptOpen = false;
    this.sessionPassword = '';
    controller.dispose();
  }

  private validate(profile: Profile): ValidationResult {
    return JSON.parse(controller.validateProfile(JSON.stringify(profile))) as ValidationResult;
  }

  private validationMessage(result: ValidationResult): string {
    switch (result.code) {
      case 'INVALID_PROFILE': return '名称或本地 ID 无效，请检查名称是否为空或过长';
      case 'INVALID_TARGET': return this.mode === 'direct' ? '请输入有效的 IP 地址和端口' : '设备 ID 应为 6 至 20 位数字';
      case 'INVALID_SERVER': return '服务器地址或端口无效';
      case 'INVALID_SERVER_KEY': return '服务器公钥无效，应为 32 字节密钥的 Base64 编码';
      case 'INVALID_FINGERPRINT': return '对端指纹应为 64 位十六进制字符';
      case 'INVALID_PEER_ID': return '被控设备 ID 应为 6 至 20 位数字';
      case 'INVALID_PEER_KEY':
      case 'INVALID_PEER_PUBLIC_KEY': return '可信公钥应为 32 字节 Ed25519 公钥的 Base64 编码';
      case 'FINGERPRINT_MISMATCH': return '对端指纹与可信公钥不一致';
      case 'INVALID_MODE': return '连接方式无效';
      case 'INVALID_JSON': return '配置字段缺失、格式错误或含有不支持的字段';
      default: return result.message ?? result.code ?? '配置无效';
    }
  }

  private probeMessage(event: ControllerEvent): string {
    switch (event.code) {
      case 'CONNECTING': return '正在连接目标地址';
      case 'TRANSPORT_OPEN': return 'TCP 连接已建立';
      case 'PROTOCOL_MESSAGE': return '已收到协议消息';
      case 'AUTHENTICATION_REQUIRED': return '网络预检结束；此检查不验证身份或登录';
      case 'TRANSPORT_FAILED': return '网络连接失败或读取失败，请检查地址与网络';
      case 'INVALID_PROTOCOL': return '目标返回的协议消息无效';
      case 'FRAME_TOO_LARGE': return '目标返回的协议帧无效或过大';
      case 'CANCELLED': return '网络预检已取消';
      default: return `网络预检状态：${event.code}`;
    }
  }

  private authenticationMessage(event: ControllerEvent): string {
    switch (event.code) {
      case 'CONNECTING': return '正在连接被控设备';
      case 'TRANSPORT_OPEN': return '传输通道已建立，正在验证身份';
      case 'IDENTITY_VERIFIED': return '设备签名与预存身份已验证，正在进行加密登录';
      case 'AUTHENTICATED': return '设备身份与加密登录已确认';
      case 'AUTHENTICATING': return '加密通道已建立，正在等待登录结果';
      case 'AWAITING_APPROVAL': return '已发送空密码请求，正在等待被控端确认';
      case 'MISSING_KEY':
      case 'MISSING_PEER_KEY':
      case 'TRUST_MATERIAL_MISSING': return '缺少预先取得的可信设备公钥';
      case 'TRUST_FAILED':
      case 'FINGERPRINT_MISMATCH':
      case 'KEY_MISMATCH': return '设备公钥或指纹与预存信任资料不一致';
      case 'INVALID_SIGNATURE':
      case 'SIGNATURE_INVALID':
      case 'IDENTITY_INVALID': return '设备签名验证失败';
      case 'IDENTITY_REQUIRED': return '被控入口未提供签名身份；旧明文入口已拒绝';
      case 'PEER_ID_MISMATCH':
      case 'INVALID_PEER_ID': return '被控设备 ID 与预存资料不一致';
      case 'UNSUPPORTED_KX_VERSION':
      case 'VERSION_MISMATCH':
      case 'KX_VERSION_UNSUPPORTED': return '对端密钥交换版本不受支持';
      case 'PLAINTEXT_REJECTED':
      case 'UNSIGNED_HANDSHAKE': return '对端未提供可信签名握手；旧明文入口已拒绝';
      case 'INVALID_CIPHERTEXT':
      case 'DECRYPT_FAILED': return '加密消息验证失败';
      case 'KEY_EXCHANGE_FAILED': return '无法建立加密会话';
      case 'INVALID_FRAME': return '对端协议帧无效或过大';
      case 'PROTOCOL_FAILED': return '加密登录协议未完成';
      case 'TRANSPORT_FAILED': return '连接或读取失败，请检查目标地址与网络';
      case 'NONCE_EXHAUSTED': return '加密会话计数耗尽';
      case 'TIMEOUT': return '加密登录超时';
      case 'PASSWORD_REJECTED':
      case 'LOGIN_REJECTED': return '登录未获接受，请检查本次密码或被控端批准状态';
      case 'SECOND_FACTOR_REQUIRED': return '被控端要求二次验证；当前页面尚未接入后续交互';
      case 'AUTH_BACKEND_NOT_READY': return '当前目标没有可用的严格认证入口';
      case 'CANCELLED': return '加密登录验证已取消';
      default: return `加密登录未完成（${event.code}）`;
    }
  }

  private async loadProfiles(): Promise<void> {
    const ownerLoad = ++this.loadId;
    this.loadActive = true;
    this.loading = true;
    this.storeError = '';
    try {
      const context = this.getUIContext().getHostContext();
      if (!context) { throw new Error('应用上下文不可用'); }
      const store = await preferences.getPreferences(context, { name: STORE_NAME });
      const raw = await store.get(STORE_KEY, '');
      if (typeof raw !== 'string') {
        throw new Error('存储内容不是文本');
      }
      const loaded = raw.length > 0 ? JSON.parse(raw) as Profile[] : [];
      if (!Array.isArray(loaded)) {
        throw new Error('存储内容不是设备列表');
      }
      const normalized: Profile[] = [];
      const ids: Set<string> = new Set<string>();
      for (let index = 0; index < loaded.length; index++) {
        const result = this.validate(loaded[index]);
        if (!result.ok || !result.profile) {
          throw new Error(`第 ${index + 1} 项无效：${this.validationMessage(result)}`);
        }
        if (ids.has(result.profile.id)) {
          throw new Error(`第 ${index + 1} 项的 ID 重复`);
        }
        ids.add(result.profile.id);
        normalized.push(result.profile);
      }
      if (ownerLoad !== this.loadId) { return; }
      this.store = store;
      this.profiles = normalized;
      if (this.selectedId.length > 0 && !normalized.some((item: Profile) => item.id === this.selectedId)) {
        this.selectedId = '';
      }
    } catch (error) {
      if (ownerLoad === this.loadId) {
        this.store = undefined;
        this.storeError = `设备资料加载失败：${String(error)}。原数据未更改。`;
      }
    } finally {
      if (ownerLoad === this.loadId) {
        this.loading = false;
        this.loadActive = false;
      }
    }
  }

  private async persist(next: Profile[]): Promise<boolean> {
    if (this.loading || this.saving || !this.store || this.storeError.length > 0) {
      this.formError = '设备资料尚未成功加载，无法保存';
      return false;
    }
    this.saving = true;
    try {
      await this.store.put(STORE_KEY, JSON.stringify(next));
      await this.store.flush();
      this.profiles = next;
      return true;
    } catch (error) {
      this.storeError = `设备资料保存失败：${String(error)}。请重新打开页面检查存储状态。`;
      return false;
    } finally {
      this.saving = false;
    }
  }

  private newProfile(): void {
    if (this.loading || this.storeError.length > 0) { return; }
    this.cancelActive();
    this.importOpen = false;
    this.importPreview = undefined;
    this.editingId = '';
    this.name = '';
    this.mode = 'direct';
    this.target = '';
    this.server = '';
    this.serverKey = '';
    this.peerFingerprint = '';
    this.peerId = '';
    this.peerPublicKey = '';
    this.formError = '';
    this.formOpen = true;
  }

  private openImport(): void {
    if (this.loading || this.storeError.length > 0) { return; }
    this.cancelActive();
    this.formOpen = false;
    this.importJson = '';
    this.importPreview = undefined;
    this.importError = '';
    this.importOpen = true;
  }

  private previewImport(): void {
    this.importPreview = undefined;
    this.importError = '';
    try {
      const parsed: Object = JSON.parse(this.importJson) as Object;
      const result = this.validate(parsed as Profile);
      if (!result.ok || !result.profile) {
        this.importError = this.validationMessage(result);
        return;
      }
      this.importPreview = result.profile;
    } catch (error) {
      this.importError = `公开配置无法解析：${String(error)}`;
    }
  }

  private async saveImport(): Promise<void> {
    if (!this.importPreview) { return; }
    const imported = this.importPreview;
    const profile: Profile = this.profiles.some((item: Profile) => item.id === imported.id)
      ? {
        id: this.generateUnusedId(),
        name: imported.name,
        mode: imported.mode,
        target: imported.target,
        server: imported.server,
        serverKey: imported.serverKey,
        peerFingerprint: imported.peerFingerprint,
        peerId: imported.peerId,
        peerPublicKey: imported.peerPublicKey
      } : imported;
    if (await this.persist(this.profiles.concat([profile]))) {
      this.selectedId = profile.id;
      this.importOpen = false;
      this.importPreview = undefined;
      this.importJson = '';
      this.status = '公开配置已保存。请核对设备 ID、公钥和目标地址后主动连接。';
    } else {
      this.importError = this.formError || this.storeError;
    }
  }

  private editProfile(profile: Profile): void {
    this.cancelActive();
    this.editingId = profile.id;
    this.name = profile.name;
    this.mode = profile.mode;
    this.target = profile.target;
    this.server = profile.server;
    this.serverKey = profile.serverKey;
    this.peerFingerprint = profile.peerFingerprint;
    this.peerId = profile.peerId ?? '';
    this.peerPublicKey = profile.peerPublicKey ?? '';
    this.formError = '';
    this.formOpen = true;
  }

  private generateId(): string {
    profileCounter++;
    return `${Date.now().toString(36)}-${profileCounter.toString(36)}`;
  }

  private generateUnusedId(): string {
    let id = this.generateId();
    while (this.profiles.some((item: Profile) => item.id === id)) { id = this.generateId(); }
    return id;
  }

  private async saveForm(): Promise<void> {
    const profile: Profile = {
      id: this.editingId.length > 0 ? this.editingId : this.generateId(),
      name: this.name.trim(),
      mode: this.mode,
      target: this.target.trim(),
      server: this.server.trim(),
      serverKey: this.serverKey.trim(),
      peerFingerprint: this.peerFingerprint.trim(),
      peerId: this.peerId.trim(),
      peerPublicKey: this.peerPublicKey.trim()
    };
    let result: ValidationResult;
    try {
      result = this.validate(profile);
    } catch (error) {
      this.formError = `配置验证失败：${String(error)}`;
      return;
    }
    if (!result.ok || !result.profile) {
      this.formError = this.validationMessage(result);
      return;
    }
    const normalized = result.profile;
    const next = this.editingId.length > 0
      ? this.profiles.map((item: Profile) => item.id === this.editingId ? normalized : item)
      : this.profiles.concat([normalized]);
    if (await this.persist(next)) {
      this.cancelActive();
      this.selectedId = normalized.id;
      this.formOpen = false;
      this.formError = '';
      this.status = normalized.mode === 'direct'
        ? '配置已保存。可主动进行网络预检；身份与授权仍未验证。'
        : '配置已保存。会话后端尚未接入。';
    }
  }

  private async deleteProfile(profile: Profile): Promise<void> {
    this.cancelActive();
    const next = this.profiles.filter((item: Profile) => item.id !== profile.id);
    if (await this.persist(next)) {
      if (this.selectedId === profile.id) { this.selectedId = ''; }
      this.status = `已删除 ${profile.name}`;
    }
  }

  private cancelActive(): void {
    const task = this.currentTask;
    this.epoch++;
    this.currentTask = 0;
    this.probing = false;
    this.activeKind = '';
    this.demoConnected = false;
    this.demoAuthorized = false;
    this.confirmationCode = '';
    this.demoX = 0;
    this.demoY = 0;
    this.demoTextLength = 0;
    this.demoText = '';
    this.demoInputStatus = '';
    this.lastPointerAt = 0;
    this.authPromptOpen = false;
    this.sessionPassword = '';
    if (task > 0) { controller.cancel(task); }
  }

  private startProbe(profile: Profile): void {
    this.cancelActive();
    if (profile.mode !== 'direct') {
      this.status = '会话后端尚未接入。';
      return;
    }
    const ownerEpoch = this.epoch;
    this.status = '网络预检中；诊断不是远程会话，未验证身份、未授权。';
    try {
      const task = controller.probeEndpoint(profile.target, 2000, (event: ControllerEvent) => {
        if (!this.visible || ownerEpoch !== this.epoch || event.taskId !== this.currentTask) { return; }
        this.status = `${this.probeMessage(event)}。诊断不是远程会话；未验证身份、未授权。`;
        if (event.state === 'blocked' || event.state === 'failed' || event.state === 'cancelled') {
          this.probing = false;
          this.activeKind = '';
          this.currentTask = 0;
          controller.cancel(event.taskId);
        }
      });
      if (task <= 0) { throw new Error('任务启动失败'); }
      this.currentTask = task;
      this.probing = true;
      this.activeKind = 'probe';
    } catch (error) {
      this.status = `网络预检无法启动：${String(error)}。请稍后重试。`;
      this.probing = false;
      this.activeKind = '';
    }
  }

  private startAuthentication(profile: Profile): void {
    const password = this.sessionPassword;
    this.sessionPassword = '';
    this.authPromptOpen = false;
    this.cancelActive();
    if (profile.mode !== 'direct' || !profile.peerId || !profile.peerPublicKey) {
      this.status = '请先保存被控设备 ID 和可信公钥。';
      return;
    }
    const request = JSON.stringify({
      endpoint: profile.target,
      peerId: profile.peerId,
      peerPublicKey: profile.peerPublicKey,
      peerFingerprint: profile.peerFingerprint.length > 0 ? profile.peerFingerprint : null,
      minimumKxVersion: 1
    });
    const ownerEpoch = this.epoch;
    this.status = '正在验证签名身份并尝试加密登录；操作仍未授权。';
    try {
      const task = controller.authenticate(request, password, 10000, (event: ControllerEvent) => {
        if (!this.visible || ownerEpoch !== this.epoch || event.taskId !== this.currentTask) { return; }
        if (event.state === 'authentication_confirmed') {
          this.status = event.verified && event.authenticated && !event.authorized
            ? '设备签名身份与加密登录已确认；操作仍未授权，无法控制设备。'
            : '登录状态不一致；操作未授权。';
        } else {
          this.status = `${this.authenticationMessage(event)}；操作未授权。`;
        }
        if (event.state === 'authentication_confirmed' || event.state === 'failed' ||
          event.state === 'blocked' || event.state === 'cancelled') {
          this.probing = false;
          this.activeKind = '';
          this.currentTask = 0;
          controller.cancel(event.taskId);
        }
      });
      if (task <= 0) { throw new Error('任务启动失败'); }
      this.currentTask = task;
      this.probing = true;
      this.activeKind = 'auth';
    } catch {
      this.status = '加密登录验证无法启动，请检查目标地址并稍后重试；操作未授权。';
      this.probing = false;
      this.activeKind = '';
    }
  }

  private startDemo(profile: Profile): void {
    this.cancelActive();
    if (profile.mode !== 'direct' || !profile.peerId || !profile.peerPublicKey) {
      this.status = '请先保存被控设备 ID 和可信公钥。';
      return;
    }
    const request = JSON.stringify({
      endpoint: profile.target,
      peerId: profile.peerId,
      peerPublicKey: profile.peerPublicKey,
      peerFingerprint: profile.peerFingerprint.length > 0 ? profile.peerFingerprint : null,
      minimumKxVersion: 1
    });
    const ownerEpoch = this.epoch;
    this.status = '正在连接 Windows 演示端；操作尚未授权。';
    try {
      const task = controller.connectDemo(request, '', 60000, (event: ControllerEvent) => {
        if (!this.visible || ownerEpoch !== this.epoch || event.taskId !== this.currentTask) { return; }
        if (event.confirmationCode) { this.confirmationCode = event.confirmationCode; }
        if (event.state === 'connected') {
          this.demoConnected = event.verified && event.authenticated;
          this.status = this.demoConnected
            ? '已登录 Windows 演示端，等待允许输入；当前只读。'
            : '登录状态不一致；操作未授权。';
        } else if (event.state === 'permissions_changed') {
          this.demoAuthorized = this.demoConnected && event.authorized;
          this.status = this.demoAuthorized ? 'Windows 演示端已允许输入。' : '输入权限已撤销；当前只读。';
        } else if (event.state === 'demo_status') {
          if (event.x !== undefined) { this.demoX = event.x; }
          if (event.y !== undefined) { this.demoY = event.y; }
          if (event.textLength !== undefined) { this.demoTextLength = event.textLength; }
          this.demoInputStatus = '演示端状态已更新。';
        } else if (event.state === 'closed' || event.state === 'cancelled' ||
          event.state === 'failed' || event.state === 'blocked') {
          this.demoAuthorized = false;
          this.demoConnected = false;
          this.status = event.state === 'closed' || event.code === 'DISCONNECTED'
            ? '演示连接已关闭；操作未授权。'
            : `${this.authenticationMessage(event)}；演示连接已结束，操作未授权。`;
          this.probing = false;
          this.activeKind = '';
          this.currentTask = 0;
          controller.cancel(event.taskId);
        } else if (event.code === 'AWAITING_APPROVAL') {
          this.status = '等待 Windows 演示端现场批准；请核对两端配对码。';
        } else {
          this.status = `${this.authenticationMessage(event)}；操作仍未授权。`;
        }
      });
      if (task <= 0) { throw new Error('任务启动失败'); }
      this.currentTask = task;
      this.probing = true;
      this.activeKind = 'demo';
    } catch {
      this.status = '演示连接无法启动，请检查目标地址后重试；操作未授权。';
      this.probing = false;
      this.activeKind = '';
    }
  }

  private startOfficialSecure(profile: Profile): void {
    this.cancelActive();
    if (profile.mode !== 'direct' || !profile.peerId || !profile.peerPublicKey) {
      this.status = '请先保存正式入口的设备 ID 和可信公钥。';
      return;
    }
    const request = JSON.stringify({
      endpoint: profile.target,
      peerId: profile.peerId,
      peerPublicKey: profile.peerPublicKey,
      peerFingerprint: profile.peerFingerprint.length > 0 ? profile.peerFingerprint : null,
      minimumKxVersion: 1
    });
    const ownerEpoch = this.epoch;
    this.status = '正在连接正式 Windows 安全入口；当前不会启动画面或输入。';
    try {
      const task = controller.connectDemo(request, '', 60000, (event: ControllerEvent) => {
        if (!this.visible || ownerEpoch !== this.epoch || event.taskId !== this.currentTask) { return; }
        if (event.state === 'connected') {
          this.demoConnected = event.verified && event.authenticated;
          this.status = this.demoConnected
            ? '正式安全入口已通过加密连接，等待 Windows 本机批准；当前只读。'
            : '正式入口登录状态不一致；操作未授权。';
        } else if (event.state === 'permissions_changed') {
          this.demoAuthorized = false;
          this.status = '正式安全入口当前只读；本阶段不开放输入。';
        } else if (event.state === 'closed' || event.state === 'cancelled' ||
          event.state === 'failed' || event.state === 'blocked') {
          this.demoConnected = false;
          this.demoAuthorized = false;
          this.status = event.code === 'DISCONNECTED' || event.state === 'closed'
            ? '正式安全入口已关闭；操作未授权。'
            : `${this.authenticationMessage(event)}；正式入口已结束。`;
          this.probing = false;
          this.activeKind = '';
          this.currentTask = 0;
          controller.cancel(event.taskId);
        } else if (event.code === 'AWAITING_APPROVAL') {
          this.status = '等待 Windows 连接管理器现场批准；当前只读。';
        } else {
          this.status = `${this.authenticationMessage(event)}；正式入口当前只读。`;
        }
      });
      if (task <= 0) { throw new Error('任务启动失败'); }
      this.currentTask = task;
      this.probing = true;
      this.activeKind = 'secure';
    } catch {
      this.status = '正式安全入口无法启动，请检查目标地址和可信公钥。';
      this.probing = false;
      this.activeKind = '';
    }
  }

  private sendDemoPointer(x: number, y: number): void {
    if (!this.demoAuthorized || !this.demoConnected || this.activeKind !== 'demo' || this.currentTask <= 0 ||
      this.padWidth <= 0 || this.padHeight <= 0) { return; }
    const now = Date.now();
    if (now - this.lastPointerAt < 50) { return; }
    this.lastPointerAt = now;
    const demoX = Math.max(0, Math.min(799, Math.floor(x * 800 / this.padWidth)));
    const demoY = Math.max(0, Math.min(449, Math.floor(y * 450 / this.padHeight)));
    if (controller.sendPointer(this.currentTask, demoX, demoY)) {
      this.demoInputStatus = '坐标已排队；以演示端状态为准。';
    } else {
      this.demoInputStatus = '坐标未发送；请检查连接和输入权限。';
    }
  }

  private utf8Length(value: string): number {
    let length = 0;
    for (let index = 0; index < value.length; index++) {
      const unit = value.charCodeAt(index);
      if (unit < 0x80) { length++; }
      else if (unit < 0x800) { length += 2; }
      else if (unit >= 0xD800 && unit <= 0xDBFF && index + 1 < value.length &&
        value.charCodeAt(index + 1) >= 0xDC00 && value.charCodeAt(index + 1) <= 0xDFFF) {
        length += 4;
        index++;
      } else if (unit >= 0xD800 && unit <= 0xDFFF) { return 513; }
      else { length += 3; }
    }
    return length;
  }

  private sendDemoText(): void {
    if (!this.demoAuthorized || !this.demoConnected || this.currentTask <= 0) { return; }
    if (this.demoText.length === 0 || this.utf8Length(this.demoText) > 512) {
      this.demoInputStatus = '文本需为 1 至 512 字节 UTF-8。';
      return;
    }
    if (controller.sendText(this.currentTask, this.demoText)) {
      this.demoInputStatus = '文本已排队；以演示端状态为准。';
      this.demoText = '';
    } else {
      this.demoInputStatus = '文本未发送；请检查连接和输入权限。';
    }
  }

  @Builder
  private field(label: string, value: string, hint: string, change: (value: string) => void) {
    Column({ space: 5 }) {
      Text(label).fontSize(14).fontColor('#29434A')
      TextInput({ text: value, placeholder: hint })
        .height(46).width('100%').backgroundColor('#FFFFFF')
        .onChange(change)
    }.width('100%').alignItems(HorizontalAlign.Start)
  }

  build() {
    Scroll() {
      Column({ space: 20 }) {
        Row() {
          Column({ space: 4 }) {
            Text('远程桌面').fontSize(25).fontWeight(FontWeight.Bold).fontColor('#17343B')
            Text('设备').fontSize(14).fontColor('#63777B')
          }.alignItems(HorizontalAlign.Start)
          Blank()
          Column({ space: 6 }) {
            Button('添加设备').enabled(!this.loading && this.storeError.length === 0 && !this.saving)
              .onClick(() => this.newProfile())
            Button('导入公开配置').enabled(!this.loading && this.storeError.length === 0 && !this.saving)
              .onClick(() => this.openImport())
          }
        }.width('100%')

        if (this.loading) {
          Text('正在加载设备资料…').fontColor('#63777B')
        }
        if (this.storeError.length > 0) {
          Column({ space: 10 }) {
            Text(this.storeError).fontColor('#B42318').width('100%')
            Button('重新加载').onClick(() => this.loadProfiles())
          }.width('100%').alignItems(HorizontalAlign.Start)
        }

        if (!this.loading && this.storeError.length === 0 && !this.formOpen && !this.importOpen) {
          if (this.profiles.length === 0) {
            Text('暂无设备').fontSize(18).fontColor('#63777B').margin({ top: 28, bottom: 28 })
          }
          ForEach(this.profiles, (profile: Profile) => {
            Column({ space: 10 }) {
              Row() {
                Column({ space: 5 }) {
                  Text(profile.name).fontSize(18).fontWeight(FontWeight.Medium).fontColor('#17343B')
                  Text(profile.mode === 'direct' ? profile.target : `${profile.mode.toUpperCase()} · ${profile.target}`)
                    .fontSize(14).fontColor('#63777B')
                }.alignItems(HorizontalAlign.Start)
                Blank()
                Text(profile.mode === 'direct' ? '直连' : profile.mode === 'id' ? '设备 ID' : '中继')
                  .fontSize(12).fontColor('#176B78')
              }.width('100%')
              if (this.selectedId === profile.id) {
                Text(profile.mode === 'direct'
                  ? '严格登录需要支持签名握手的被控入口；旧明文入口会被拒绝。'
                  : '会话后端尚未接入。')
                  .fontSize(13).fontColor('#63777B').width('100%')
                Flex({ direction: FlexDirection.Row, wrap: FlexWrap.Wrap }) {
                  if (profile.mode === 'direct') {
                    Button('网络预检').margin({ right: 8, bottom: 8 })
                      .enabled(!this.probing && !this.saving).onClick(() => this.startProbe(profile))
                    if (profile.peerId && profile.peerPublicKey) {
                      Button('验证加密登录').margin({ right: 8, bottom: 8 })
                        .enabled(!this.probing && !this.saving)
                        .onClick(() => { this.authPromptOpen = true; this.sessionPassword = ''; })
                      Button('连接演示端').margin({ right: 8, bottom: 8 })
                        .enabled(!this.probing && !this.saving).onClick(() => this.startDemo(profile))
                      Button('连接正式入口').margin({ right: 8, bottom: 8 })
                        .enabled(!this.probing && !this.saving).onClick(() => this.startOfficialSecure(profile))
                    }
                  }
                  Button('编辑').margin({ right: 8, bottom: 8 }).enabled(!this.saving)
                    .onClick(() => this.editProfile(profile))
                  Button('删除').margin({ right: 8, bottom: 8 }).enabled(!this.saving)
                    .onClick(() => this.deleteProfile(profile))
                  if (this.probing && profile.mode === 'direct') {
                    Button(this.activeKind === 'demo' || this.activeKind === 'secure' ? '断开连接' :
                      this.activeKind === 'auth' ? '取消登录验证' : '取消预检').margin({ bottom: 8 })
                      .onClick(() => { this.cancelActive(); this.status = '连接或检查已取消；操作未授权。'; })
                  }
                }.width('100%')
                if (this.authPromptOpen && profile.mode === 'direct') {
                  Column({ space: 10 }) {
                    Text('本次登录密码').fontSize(14).fontColor('#29434A').width('100%')
                    TextInput({ text: this.sessionPassword, placeholder: '可留空，由被控端现场批准' })
                      .type(InputType.Password).height(46).width('100%').backgroundColor('#F4F7F8')
                      .onChange((value: string) => { this.sessionPassword = value; })
                    Row({ space: 10 }) {
                      Button('开始验证').onClick(() => this.startAuthentication(profile))
                      Button('取消').onClick(() => { this.sessionPassword = ''; this.authPromptOpen = false; })
                    }.width('100%')
                  }.width('100%').alignItems(HorizontalAlign.Start)
                }
                if (this.activeKind === 'demo' && profile.mode === 'direct') {
                  Column({ space: 10 }) {
                    if (this.confirmationCode.length > 0) {
                      Text(`配对码 ${this.confirmationCode}`).fontSize(18).fontWeight(FontWeight.Bold)
                        .fontColor('#17343B').width('100%')
                      Text(this.demoConnected
                        ? '配对码已用于本次连接核对；输入权限由 Windows 演示端单独授予。'
                        : '请与 Windows 演示端显示的配对码核对，并等待现场批准。')
                        .fontSize(13).fontColor('#63777B').width('100%')
                    }
                    Text(this.demoAuthorized ? '允许输入' : this.demoConnected ? '只读，等待允许输入' : '等待连接和批准')
                      .fontSize(14).fontColor(this.demoAuthorized ? '#176B78' : '#63777B').width('100%')
                    Text('操作只影响 Windows 演示窗口；尚无视频画面。')
                      .fontSize(13).fontColor('#63777B').width('100%')
                    Stack() {
                      Text(this.demoAuthorized ? '演示操作区' : '演示操作区 · 只读')
                        .fontSize(14).fontColor('#63777B')
                    }.width('100%').aspectRatio(800 / 450).backgroundColor('#E4ECEE')
                      .border({ width: 1, color: '#B6C9CD' })
                      .onAreaChange((oldArea: Area, area: Area) => {
                        this.padWidth = parseFloat(String(area.width));
                        this.padHeight = parseFloat(String(area.height));
                      })
                      .onTouch((event: TouchEvent) => {
                        if (event.type === TouchType.Down || event.type === TouchType.Move) {
                          if (event.touches.length > 0) {
                            this.sendDemoPointer(event.touches[0].x, event.touches[0].y);
                          }
                        }
                      })
                    Text(`演示端反馈：坐标 (${this.demoX}, ${this.demoY}) · 文本长度 ${this.demoTextLength}`)
                      .fontSize(13).fontColor('#29434A').width('100%')
                    TextInput({ text: this.demoText, placeholder: '输入发送到演示窗口的文字' })
                      .height(46).width('100%').backgroundColor('#F4F7F8')
                      .enabled(this.demoAuthorized)
                      .onChange((value: string) => { this.demoText = value; })
                    Button('发送文本').enabled(this.demoAuthorized && this.demoText.length > 0)
                      .onClick(() => this.sendDemoText())
                    if (this.demoInputStatus.length > 0) {
                      Text(this.demoInputStatus).fontSize(13).fontColor('#63777B').width('100%')
                    }
                  }.width('100%').alignItems(HorizontalAlign.Start)
                }
                if (this.activeKind === 'secure' && profile.mode === 'direct') {
                  Column({ space: 10 }) {
                    Text(this.demoConnected ? '正式安全入口已连接 · 只读' : '等待正式入口批准')
                      .fontSize(15).fontColor('#176B78').width('100%')
                    Text('当前切片只验证签名、加密和现场批准，不启动远程画面或系统输入。')
                      .fontSize(13).fontColor('#63777B').width('100%')
                    Text('请在 Windows 被控端观察连接管理器批准请求。')
                      .fontSize(13).fontColor('#63777B').width('100%')
                  }.width('100%').alignItems(HorizontalAlign.Start)
                }
              }
            }.padding(14).width('100%').borderRadius(6).backgroundColor('#FFFFFF')
              .onClick(() => {
                if (this.selectedId !== profile.id) {
                  this.cancelActive();
                  this.status = '';
                }
                this.selectedId = profile.id;
              })
          }, (profile: Profile) => profile.id)
        }

        if (this.importOpen && !this.loading && this.storeError.length === 0) {
          Column({ space: 12 }) {
            Text('导入公开配置').fontSize(19).fontWeight(FontWeight.Medium).fontColor('#17343B')
              .width('100%')
            TextArea({ text: this.importJson, placeholder: '粘贴 Windows 演示端导出的公开 profile JSON' })
              .height(150).width('100%').backgroundColor('#FFFFFF')
              .onChange((value: string) => {
                this.importJson = value;
                this.importPreview = undefined;
                this.importError = '';
              })
            Button('验证并预览').enabled(!this.saving && this.importJson.length > 0)
              .onClick(() => this.previewImport())
            if (this.importPreview) {
              Text(`名称：${this.importPreview.name}`).width('100%')
              Text(`目标地址：${this.importPreview.target}`).width('100%')
              Text(`设备 ID：${this.importPreview.peerId ?? ''}`).width('100%')
              Text(`可信公钥：${this.importPreview.peerPublicKey ?? ''}`).width('100%')
              Text(`指纹：${this.importPreview.peerFingerprint}`).width('100%')
              Text('仅保存公开资料；请自行核对可信来源。保存后不会自动连接。')
                .fontSize(13).fontColor('#63777B').width('100%')
              Button('确认保存').enabled(!this.saving).onClick(() => this.saveImport())
            }
            if (this.importError.length > 0) {
              Text(this.importError).fontColor('#B42318').width('100%')
            }
            Button('取消').enabled(!this.saving).onClick(() => {
              this.importOpen = false;
              this.importPreview = undefined;
              this.importJson = '';
            })
          }.width('100%').alignItems(HorizontalAlign.Start)
        }

        if (this.formOpen && !this.loading && this.storeError.length === 0) {
          Column({ space: 15 }) {
            Text(this.editingId.length > 0 ? '编辑设备' : '添加设备')
              .fontSize(19).fontWeight(FontWeight.Medium).fontColor('#17343B').width('100%')
            this.field('名称', this.name, '例如：办公室电脑', (value: string) => { this.name = value; })
            Text('连接方式').fontSize(14).fontColor('#29434A').width('100%')
            Row({ space: 8 }) {
              Button('IP 直连').backgroundColor(this.mode === 'direct' ? '#176B78' : '#DEE9EB')
                .fontColor(this.mode === 'direct' ? '#FFFFFF' : '#29434A')
                .onClick(() => { this.mode = 'direct'; })
              Button('设备 ID').backgroundColor(this.mode === 'id' ? '#176B78' : '#DEE9EB')
                .fontColor(this.mode === 'id' ? '#FFFFFF' : '#29434A')
                .onClick(() => { this.mode = 'id'; })
              Button('中继').backgroundColor(this.mode === 'relay' ? '#176B78' : '#DEE9EB')
                .fontColor(this.mode === 'relay' ? '#FFFFFF' : '#29434A')
                .onClick(() => { this.mode = 'relay'; })
            }.width('100%')
            this.field(this.mode === 'direct' ? 'IP 地址与端口' : '设备 ID', this.target,
              this.mode === 'direct' ? '192.168.1.10:21118 或 [::1]:21118' : '6 至 20 位数字',
              (value: string) => { this.target = value; })
            if (this.mode !== 'direct') {
              this.field('服务器地址', this.server, '域名或地址', (value: string) => { this.server = value; })
              this.field('服务器公钥', this.serverKey, '可选；请核对可信来源', (value: string) => { this.serverKey = value; })
            } else {
              this.field('被控设备 ID', this.peerId, '6 至 20 位数字', (value: string) => { this.peerId = value; })
              this.field('可信 Ed25519 公钥', this.peerPublicKey, '32 字节公钥的 Base64 编码',
                (value: string) => { this.peerPublicKey = value; })
              Text('公钥只能从可信来源取得；网络预检不会自动建立信任。')
                .fontSize(13).fontColor('#63777B').width('100%')
            }
            this.field('对端 SHA-256 指纹', this.peerFingerprint, '可选；与可信公钥对应的 64 位十六进制',
              (value: string) => { this.peerFingerprint = value; })
            if (this.formError.length > 0) {
              Text(this.formError).fontColor('#B42318').width('100%')
            }
            Row({ space: 10 }) {
              Button('保存').enabled(!this.saving).onClick(() => this.saveForm())
              Button('取消').enabled(!this.saving).onClick(() => { this.formOpen = false; this.formError = ''; })
            }.width('100%')
          }.width('100%').alignItems(HorizontalAlign.Start)
        }

        if (this.status.length > 0) {
          Text(this.status).fontSize(14).fontColor('#29434A').width('100%')
        }
        Text(this.demoAuthorized ? '操作权限：仅 Windows 演示窗口允许输入。' :
          '操作权限：未授权。身份验证与登录确认不提供远程控制。')
          .fontSize(13).fontColor('#63777B').width('100%')
      }.width('100%').constraintSize({ maxWidth: 760 }).padding({ left: 18, right: 18, top: 24, bottom: 32 })
    }.width('100%').height('100%').backgroundColor('#F4F7F8')
  }
}
