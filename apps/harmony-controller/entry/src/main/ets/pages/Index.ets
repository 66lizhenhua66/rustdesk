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
  @State private formError: string = '';
  @State private selectedId: string = '';
  @State private status: string = '';
  @State private probing: boolean = false;
  private store?: preferences.Preferences;
  private currentTask: number = 0;
  private epoch: number = 0;
  private visible: boolean = true;
  private loadId: number = 0;
  private loadActive: boolean = false;

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
    if (this.probing) { this.status = '网络预检已停止；身份未验证，操作未授权。'; }
    this.visible = false;
    this.epoch++;
    this.loadId++;
    this.loadActive = false;
    this.currentTask = 0;
    this.probing = false;
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
      case 'AUTH_BACKEND_NOT_READY': return '网络预检结束；认证会话后端尚未接入';
      case 'TRANSPORT_FAILED': return '网络连接失败或读取失败，请检查地址与网络';
      case 'INVALID_PROTOCOL': return '目标返回的协议消息无效';
      case 'FRAME_TOO_LARGE': return '目标返回的协议帧无效或过大';
      case 'CANCELLED': return '网络预检已取消';
      default: return `网络预检状态：${event.code}`;
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
    this.editingId = '';
    this.name = '';
    this.mode = 'direct';
    this.target = '';
    this.server = '';
    this.serverKey = '';
    this.peerFingerprint = '';
    this.formError = '';
    this.formOpen = true;
  }

  private editProfile(profile: Profile): void {
    this.cancelProbe();
    this.editingId = profile.id;
    this.name = profile.name;
    this.mode = profile.mode;
    this.target = profile.target;
    this.server = profile.server;
    this.serverKey = profile.serverKey;
    this.peerFingerprint = profile.peerFingerprint;
    this.formError = '';
    this.formOpen = true;
  }

  private generateId(): string {
    profileCounter++;
    return `${Date.now().toString(36)}-${profileCounter.toString(36)}`;
  }

  private async saveForm(): Promise<void> {
    const profile: Profile = {
      id: this.editingId.length > 0 ? this.editingId : this.generateId(),
      name: this.name.trim(),
      mode: this.mode,
      target: this.target.trim(),
      server: this.server.trim(),
      serverKey: this.serverKey.trim(),
      peerFingerprint: this.peerFingerprint.trim()
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
      this.cancelProbe();
      this.selectedId = normalized.id;
      this.formOpen = false;
      this.formError = '';
      this.status = normalized.mode === 'direct'
        ? '配置已保存。可主动进行网络预检；身份与授权仍未验证。'
        : '配置已保存。会话后端尚未接入。';
    }
  }

  private async deleteProfile(profile: Profile): Promise<void> {
    this.cancelProbe();
    const next = this.profiles.filter((item: Profile) => item.id !== profile.id);
    if (await this.persist(next)) {
      if (this.selectedId === profile.id) { this.selectedId = ''; }
      this.status = `已删除 ${profile.name}`;
    }
  }

  private cancelProbe(): void {
    const task = this.currentTask;
    this.epoch++;
    this.currentTask = 0;
    this.probing = false;
    if (task > 0) { controller.cancel(task); }
  }

  private startProbe(profile: Profile): void {
    this.cancelProbe();
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
          this.currentTask = 0;
          controller.cancel(event.taskId);
        }
      });
      if (task <= 0) { throw new Error('任务启动失败'); }
      this.currentTask = task;
      this.probing = true;
    } catch (error) {
      this.status = `网络预检无法启动：${String(error)}。请稍后重试。`;
      this.probing = false;
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
          Button('添加设备').enabled(!this.loading && this.storeError.length === 0 && !this.saving)
            .onClick(() => this.newProfile())
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

        if (!this.loading && this.storeError.length === 0 && !this.formOpen) {
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
                Text(profile.mode === 'direct' ? '网络预检仅检查目标可达性与协议响应。' : '会话后端尚未接入。')
                  .fontSize(13).fontColor('#63777B').width('100%')
                Flex({ direction: FlexDirection.Row, wrap: FlexWrap.Wrap }) {
                  if (profile.mode === 'direct') {
                    Button('网络预检').margin({ right: 8, bottom: 8 })
                      .enabled(!this.probing && !this.saving).onClick(() => this.startProbe(profile))
                  }
                  Button('编辑').margin({ right: 8, bottom: 8 }).enabled(!this.saving)
                    .onClick(() => this.editProfile(profile))
                  Button('删除').margin({ right: 8, bottom: 8 }).enabled(!this.saving)
                    .onClick(() => this.deleteProfile(profile))
                  if (this.probing && profile.mode === 'direct') {
                    Button('取消预检').margin({ bottom: 8 })
                      .onClick(() => { this.cancelProbe(); this.status = '网络预检已取消。'; })
                  }
                }.width('100%')
              }
            }.padding(14).width('100%').borderRadius(6).backgroundColor('#FFFFFF')
              .onClick(() => { this.selectedId = profile.id; })
          }, (profile: Profile) => profile.id)
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
            }
            this.field('对端指纹', this.peerFingerprint, '可选；64 位十六进制',
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
        Text('诊断不是远程会话；身份未验证，操作未授权。')
          .fontSize(13).fontColor('#63777B').width('100%')
      }.width('100%').constraintSize({ maxWidth: 760 }).padding({ left: 18, right: 18, top: 24, bottom: 32 })
    }.width('100%').height('100%').backgroundColor('#F4F7F8')
  }
}
