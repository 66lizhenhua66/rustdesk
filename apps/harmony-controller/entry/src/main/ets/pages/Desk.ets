import controller, { ControllerEvent } from 'libcontroller.so';
import { preferences } from '@kit.ArkData';
import { RemoteSession } from '../components/RemoteSession';
import { Profile, ScreenState, initialScreenState, projectScreenEvent, endedScreenState,
  screenIsLive, splitEndpoint, joinEndpoint, canConnectProfile } from '../model/DeskModels';

interface ValidationResult {
  ok: boolean;
  profile?: Profile;
  code?: string;
}

interface RecentConnection {
  profileId: string;
  name: string;
  target: string;
  at: number;
}

class DeskSurface extends XComponentController {
  private created: (id: string) => void;
  private destroyed: (id: string) => void;
  constructor(created: (id: string) => void, destroyed: (id: string) => void) {
    super();
    this.created = created;
    this.destroyed = destroyed;
  }
  onSurfaceCreated(id: string): void { this.created(id); }
  onSurfaceDestroyed(id: string): void { this.destroyed(id); }
}

@Entry
@Component
struct Desk {
  @State private profiles: Profile[] = [];
  @State private favorites: string[] = [];
  @State private recent: RecentConnection[] = [];
  @State private loading: boolean = true;
  @State private saving: boolean = false;
  @State private storeError: string = '';
  @State private notice: string = '';
  @State private page: string = 'devices';
  @State private search: string = '';
  @State private onlyFavorites: boolean = false;
  @State private selectedId: string = '';
  @State private windowWidth: number = 390;
  @State private modal: string = '';
  @State private editingId: string = '';
  @State private name: string = '';
  @State private address: string = '';
  @State private port: string = '21120';
  @State private peerId: string = '';
  @State private peerPublicKey: string = '';
  @State private fingerprint: string = '';
  @State private formError: string = '';
  @State private importJson: string = '';
  @State private importPreview: Profile | undefined = undefined;
  @State private sessionProfile: Profile | undefined = undefined;
  @State private sessionOpen: boolean = false;
  @State private sessionKey: number = 0;
  @State private screen: ScreenState = new ScreenState();
  private surfaceController: XComponentController = new XComponentController();
  private store?: preferences.Preferences;
  private task: number = 0;
  private generation: number = 0;
  private loadGeneration: number = 0;
  private visible: boolean = true;
  private surfaceId: string = '';

  aboutToAppear(): void { this.visible = true; this.loadProfiles(); }
  onPageShow(): void { this.visible = true; this.loadProfiles(); }
  onPageHide(): void { this.leavePage(); }
  aboutToDisappear(): void { this.leavePage(); }

  onBackPress(): boolean {
    if (this.modal.length > 0 && !this.saving) { this.modal = ''; return true; }
    if (this.sessionOpen) { this.sessionOpen = false; return true; }
    return false;
  }

  private leavePage(): void {
    this.visible = false;
    this.loadGeneration++;
    if (screenIsLive(this.screen)) { this.disconnect('应用已离开前台，连接已结束。请重新连接并等待本机批准。'); }
    controller.dispose();
  }

  private validate(profile: Profile): ValidationResult {
    return JSON.parse(controller.validateProfile(JSON.stringify(profile))) as ValidationResult;
  }

  private validationMessage(code: string): string {
    switch (code) {
      case 'INVALID_PROFILE': return '请填写设备名称，并检查名称是否过长。';
      case 'INVALID_TARGET': return '请输入有效的 IPv4 / IPv6 地址和 1–65535 端口。';
      case 'INVALID_PEER_ID': return '被控设备 ID 应为 6 至 20 位数字。';
      case 'INVALID_PEER_KEY':
      case 'INVALID_PEER_PUBLIC_KEY': return '可信公钥应为 32 字节 Ed25519 公钥的 Base64 编码。';
      case 'INVALID_FINGERPRINT': return '指纹应为 64 位十六进制字符，或留空。';
      case 'FINGERPRINT_MISMATCH': return '指纹与可信公钥不一致，请重新核对。';
      case 'INVALID_SERVER': return '服务器地址无效。';
      case 'INVALID_SERVER_KEY': return '服务器公钥无效。';
      default: return `资料校验未通过（${code}），请检查公开配置。`;
    }
  }

  private async loadProfiles(): Promise<void> {
    const owner = ++this.loadGeneration;
    this.loading = true;
    try {
      const context = this.getUIContext().getHostContext();
      if (!context) { throw new Error('应用上下文不可用'); }
      const store = await preferences.getPreferences(context, { name: 'controller_profiles' });
      const raw = await store.get('profiles', '');
      if (typeof raw !== 'string') { throw new Error('设备资料不是文本'); }
      const loaded: Profile[] = raw.length > 0 ? JSON.parse(raw) as Profile[] : [];
      if (!Array.isArray(loaded)) { throw new Error('设备资料不是列表'); }
      const normalized: Profile[] = [];
      const ids: Set<string> = new Set<string>();
      for (let i = 0; i < loaded.length; i++) {
        const result = this.validate(loaded[i]);
        if (!result.ok || !result.profile || ids.has(result.profile.id)) {
          throw new Error(`第 ${i + 1} 项设备资料无效或 ID 重复`);
        }
        ids.add(result.profile.id);
        normalized.push(result.profile);
      }
      const favoriteRaw = await store.get('desk_favorites', '[]');
      const recentRaw = await store.get('desk_recent', '[]');
      const favorites = JSON.parse(String(favoriteRaw)) as string[];
      const recent = JSON.parse(String(recentRaw)) as RecentConnection[];
      if (!Array.isArray(favorites) || !Array.isArray(recent)) { throw new Error('工作台记录格式无效'); }
      if (owner !== this.loadGeneration || !this.visible) { return; }
      this.store = store;
      this.profiles = normalized;
      this.favorites = favorites;
      this.recent = recent.slice(0, 30);
      this.storeError = '';
      if (!normalized.some((p: Profile) => p.id === this.selectedId)) {
        const ready = normalized.find((p: Profile) => canConnectProfile(p));
        this.selectedId = ready ? ready.id : normalized.length > 0 ? normalized[0].id : '';
      }
    } catch {
      if (owner === this.loadGeneration) {
        this.store = undefined;
        this.storeError = '无法读取设备资料，原数据已保留。请重新加载或进入连接诊断检查。';
      }
    } finally {
      if (owner === this.loadGeneration) { this.loading = false; }
    }
  }

  private async persistProfiles(next: Profile[]): Promise<boolean> {
    if (!this.store || this.loading || this.saving || this.storeError.length > 0) { return false; }
    this.saving = true;
    try {
      await this.store.put('profiles', JSON.stringify(next));
      await this.store.flush();
      this.profiles = next;
      return true;
    } catch {
      this.storeError = '设备资料保存失败。请重新加载检查存储状态。';
      this.formError = this.storeError;
      return false;
    } finally { this.saving = false; }
  }

  private async toggleFavorite(id: string): Promise<void> {
    if (!this.store || this.saving) { return; }
    const next = this.favorites.includes(id) ? this.favorites.filter((item: string) => item !== id) : this.favorites.concat([id]);
    this.saving = true;
    try {
      await this.store.put('desk_favorites', JSON.stringify(next));
      await this.store.flush();
      this.favorites = next;
    } catch { this.notice = '常用设备未能保存，请稍后重试。'; }
    finally { this.saving = false; }
  }

  private async recordConnection(profile: Profile): Promise<void> {
    if (!this.store) { return; }
    const record: RecentConnection = { profileId: profile.id, name: profile.name, target: profile.target, at: Date.now() };
    const next = [record].concat(this.recent).slice(0, 30);
    this.recent = next;
    try {
      await this.store.put('desk_recent', JSON.stringify(next));
      await this.store.flush();
    } catch { this.notice = '本次连接记录未能保存；当前会话不受影响。'; }
  }

  private openEditor(profile?: Profile): void {
    if (this.saving || this.loading || this.storeError.length > 0) { return; }
    if (profile && this.sessionProfile?.id === profile.id && screenIsLive(this.screen)) {
      this.notice = '请先断开此设备的会话，再修改连接资料。';
      return;
    }
    if (profile && profile.mode !== 'direct') {
      this.notice = '设备 ID / 中继资料可在偏好设置的连接诊断中编辑；当前尚不能建立桌面会话。';
      return;
    }
    this.editingId = profile?.id ?? '';
    this.name = profile?.name ?? '';
    const parts = splitEndpoint(profile?.target ?? '');
    this.address = parts.address;
    this.port = parts.port;
    this.peerId = profile?.peerId ?? '';
    this.peerPublicKey = profile?.peerPublicKey ?? '';
    this.fingerprint = profile?.peerFingerprint ?? '';
    this.formError = '';
    this.modal = 'edit';
  }

  private async saveForm(): Promise<void> {
    if (this.saving) { return; }
    const previous = this.profiles.find((p: Profile) => p.id === this.editingId);
    const profile: Profile = {
      id: this.editingId || `desk-${Date.now().toString(36)}`,
      name: this.name.trim(), mode: 'direct', target: joinEndpoint(this.address, this.port),
      server: previous?.server ?? '', serverKey: previous?.serverKey ?? '', peerFingerprint: this.fingerprint.trim(),
      peerId: this.peerId.trim(), peerPublicKey: this.peerPublicKey.trim()
    };
    const result = this.validate(profile);
    if (!result.ok || !result.profile) { this.formError = this.validationMessage(result.code ?? 'INVALID_PROFILE'); return; }
    const normalized = result.profile;
    const next = this.editingId ? this.profiles.map((p: Profile) => p.id === this.editingId ? normalized : p) : this.profiles.concat([normalized]);
    if (await this.persistProfiles(next)) {
      this.selectedId = normalized.id;
      this.modal = '';
      this.notice = '设备已保存。连接前请核对地址和可信身份。';
    }
  }

  private openImport(): void {
    this.importJson = ''; this.importPreview = undefined; this.formError = ''; this.modal = 'import';
  }

  private previewImport(): void {
    this.importPreview = undefined;
    try {
      const result = this.validate(JSON.parse(this.importJson) as Profile);
      if (!result.ok || !result.profile) { this.formError = this.validationMessage(result.code ?? 'INVALID_JSON'); return; }
      this.importPreview = result.profile;
      this.formError = '';
    } catch { this.formError = '配置不是有效的公开设备 JSON，请检查内容。'; }
  }

  private async saveImport(): Promise<void> {
    if (!this.importPreview) { return; }
    const profile = this.importPreview;
    if (this.profiles.some((p: Profile) => p.id === profile.id)) {
      this.formError = '已有相同本地 ID 的设备，请编辑原设备；导入不会覆盖已有信任资料。';
      return;
    }
    if (await this.persistProfiles(this.profiles.concat([profile]))) {
      this.selectedId = profile.id; this.modal = ''; this.importJson = ''; this.importPreview = undefined;
      this.notice = '公开配置已保存，未自动连接。请核对设备身份后主动连接。';
    }
  }

  private async deleteDevice(): Promise<void> {
    if (this.sessionProfile?.id === this.editingId && screenIsLive(this.screen)) {
      this.formError = '请先断开此设备的会话。'; return;
    }
    if (await this.persistProfiles(this.profiles.filter((p: Profile) => p.id !== this.editingId))) {
      if (this.selectedId === this.editingId) { this.selectedId = ''; }
      this.modal = ''; this.notice = '设备已从本机移除。';
    }
  }

  private selectedProfile(): Profile | undefined { return this.profiles.find((p: Profile) => p.id === this.selectedId); }
  private filteredProfiles(): Profile[] {
    const query = this.search.trim().toLowerCase();
    return this.profiles.filter((p: Profile) => (!this.onlyFavorites || this.favorites.includes(p.id)) &&
      `${p.name} ${p.target} ${p.peerId ?? ''}`.toLowerCase().includes(query));
  }

  private connectSelected(): void {
    const profile = this.selectedProfile();
    if (profile) { this.connect(profile); }
    else { this.openEditor(); }
  }

  private connect(profile: Profile): void {
    if (screenIsLive(this.screen)) {
      if (this.sessionProfile?.id === profile.id) { this.sessionOpen = true; }
      else { this.notice = '已有设备正在连接或共享画面，请先返回会话并断开。'; }
      return;
    }
    if (!canConnectProfile(profile)) { this.notice = '此设备尚不能连接，请完善 IP 直连的可信设备 ID 和公钥。'; return; }
    this.disconnect('');
    const owner = this.generation;
    this.selectedId = profile.id;
    this.sessionProfile = profile;
    this.surfaceController = new DeskSurface((id: string) => {
      if (!this.visible || this.generation !== owner || !screenIsLive(this.screen) || this.task !== 0) { return; }
      this.surfaceId = id;
      this.startScreen(profile, id, owner);
    }, (id: string) => {
      if (this.generation === owner && this.surfaceId === id) {
        this.disconnect('显示区域已关闭，连接已结束。请重新连接。');
      }
    });
    this.screen = initialScreenState();
    this.sessionKey++;
    this.sessionOpen = true;
    this.notice = '';
  }

  private startScreen(profile: Profile, surfaceId: string, owner: number): void {
    try {
      const request = JSON.stringify({ endpoint: profile.target, peerId: profile.peerId,
        peerPublicKey: profile.peerPublicKey, peerFingerprint: profile.peerFingerprint || null,
        minimumKxVersion: 1, expectedPeer: 'secure_video' });
      const task = controller.connectScreen(request, surfaceId, 60000, (event: ControllerEvent) => {
        if (!this.visible || owner !== this.generation || event.taskId !== this.task) { return; }
        const wasApproved = this.screen.approved;
        this.screen = projectScreenEvent(this.screen, event);
        if (!wasApproved && this.screen.approved) { this.recordConnection(profile); }
        if (!screenIsLive(this.screen)) {
          this.generation++;
          this.task = 0;
          this.surfaceId = '';
          controller.cancel(event.taskId);
        }
      });
      if (task <= 0) { throw new Error('无法创建会话'); }
      this.task = task;
    } catch {
      this.disconnect('无法启动画面连接，请检查设备资料后重试。', 'START_FAILED');
    }
  }

  private disconnect(detail: string = '已停止共享画面。本次批准已失效，重新连接需要 Windows 本机再次批准。', code: string = 'CANCELLED'): void {
    const task = this.task;
    this.generation++;
    this.task = 0;
    this.surfaceId = '';
    this.screen = endedScreenState(detail, code);
    if (task > 0) { controller.cancel(task); }
  }

  private openDiagnostics(): void {
    if (screenIsLive(this.screen)) { this.notice = '请先断开当前桌面会话，再进入连接诊断。'; return; }
    this.getUIContext().getRouter().pushUrl({ url: 'pages/Index' }).catch(() => { this.notice = '连接诊断页面无法打开。'; });
  }

  @Builder
  private action(label: string, click: () => void, primary: boolean = false, enabled: () => boolean = () => true) {
    Button(label, { type: ButtonType.Normal }).fontSize(14).fontWeight(FontWeight.Medium)
      .fontColor(primary ? '#FFFFFF' : '#196B52').backgroundColor(primary ? '#196B52' : '#F0F5EF')
      .height(44).borderRadius(10).padding({ left: 16, right: 16 }).enabled(enabled()).onClick(click)
  }

  @Builder
  private navItem(id: string, icon: Resource, label: string) {
    Button({ type: ButtonType.Normal }) {
      Column({ space: 5 }) {
        Image(icon).width(22).height(22).fillColor(this.windowWidth >= 600 ? '#DCEAD6' : '#196B52')
        if (this.windowWidth < 600 || this.windowWidth >= 1100) {
          Text(label).fontSize(12).fontColor(this.windowWidth >= 600 ? '#DCEAD6' : '#20352F')
        }
      }.padding(8)
    }.height(this.windowWidth < 600 ? 60 : 64).width(this.windowWidth < 600 ? '32%' : '100%')
      .borderRadius(10).backgroundColor(this.page === id ? (this.windowWidth >= 600 ? '#395446' : '#E5F2E9') : Color.Transparent)
      .accessibilityText(label).onClick(() => { this.page = id; this.notice = ''; })
  }

  @Builder
  private sideNav() {
    Column({ space: 22 }) {
      Text('↗').fontSize(32).fontColor('#D2ED9A').margin({ top: 28, bottom: 16 })
      if (this.windowWidth >= 1100) { Text('Remote Desk').fontSize(18).fontColor('#F8FCF8') }
      this.navItem('devices', $r('app.media.desk_devices'), '我的设备')
      this.navItem('recent', $r('app.media.desk_recent'), '最近连接')
      this.navItem('settings', $r('app.media.desk_session_settings'), '偏好设置')
      Blank()
      Text(this.windowWidth >= 1100 ? 'HARMONY OS\n本机工作空间' : 'RD').fontSize(11).fontColor('#A8BCB0').textAlign(TextAlign.Center).margin({ bottom: 26 })
    }.width(this.windowWidth >= 1100 ? 196 : 75).height('100%').padding({ left: 12, right: 12 }).backgroundColor('#20382E')
  }

  @Builder
  private welcome() {
    Stack({ alignContent: Alignment.BottomEnd }) {
      Column({ space: 14 }) {
        Text('A LITTLE CLOSER, EVERY DAY').fontSize(10).letterSpacing(1.5).fontColor('#6B835E')
        Text('下一站，\n你的桌面。').fontSize(30).fontWeight(FontWeight.Medium).fontColor('#334C34').lineHeight(42)
        Text('熟悉的工作空间，\n就在手边。').fontSize(13).lineHeight(23).fontColor('#7A8C6D')
        Text('现场批准 · 安心查看').fontSize(11).fontColor('#69815E').margin({ top: 8 })
      }.width('100%').height('100%').alignItems(HorizontalAlign.Start).justifyContent(FlexAlign.Start).padding(24)
      Image($r('app.media.desk_welcome')).width(158).height(155).margin({ right: 0, bottom: 0 }).opacity(0.9)
    }.width('100%').height(this.windowWidth < 600 ? 236 : 354).borderRadius(18).clip(true).backgroundColor('#E6EEE1')
  }

  @Builder
  private connectionCard() {
    Column({ space: 14 }) {
      Text('↗  连接远程设备').fontSize(17).fontWeight(FontWeight.Medium).fontColor('#20352F')
      Row({ space: 18 }) {
        Text('设备 ID · 未实现').fontSize(11).fontColor('#9CA79F')
        Text('IP 直连').fontSize(13).fontWeight(FontWeight.Medium).fontColor('#196B52')
        Text('中继 · 未实现').fontSize(11).fontColor('#9CA79F')
      }.width('100%').height(34).border({ width: { bottom: 1 }, color: '#E4EAE5' })
      if (this.profiles.length > 0) {
        Select(this.profiles.map((p: Profile): SelectOption => { return { value: p.name }; }))
          .value(this.selectedProfile()?.name ?? '选择已保存的设备').width('100%').height(46)
          .backgroundColor('#F6F8F5').font({ size: 14 }).fontColor('#20352F')
          .onSelect((index: number) => { this.selectedId = this.profiles[index].id; })
        Text(this.selectedProfile()?.target ?? '请先选择设备').fontSize(13).fontColor('#778780').maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
      } else {
        Text('先添加一台你的电脑').fontSize(17).fontColor('#20352F').margin({ top: 6 })
        Text('保存 IP 地址和可信设备身份，随时从这里连接。').fontSize(13).fontColor('#778780').lineHeight(22)
      }
      Row({ space: 8 }) {
        Text('●  现场批准').fontSize(12).fontColor('#196B52')
        Text('无人值守 · 未实现').fontSize(11).fontColor('#9CA79F')
      }.width('100%')
      Text('进入桌面后：仅查看').fontSize(13).fontColor('#53695C')
      Button(this.profiles.length === 0 ? '添加设备  ＋' : '连接桌面  →', { type: ButtonType.Normal })
        .width('100%').height(46).borderRadius(10).backgroundColor('#196B52').fontColor('#FFFFFF').fontSize(14)
        .enabled(!this.loading && !this.saving && this.storeError.length === 0 &&
          (this.profiles.length === 0 || (this.selectedProfile() !== undefined && canConnectProfile(this.selectedProfile()!))))
        .onClick(() => this.connectSelected())
      Text('每次连接，都由 Windows 本机批准查看。').fontSize(11).fontColor('#778780')
    }.width('100%').padding(24).alignItems(HorizontalAlign.Start).backgroundColor('#FFFFFF')
      .border({ width: 1, color: '#E4EAE5' }).borderRadius(18).constraintSize({ minHeight: this.windowWidth >= 600 ? 354 : 0 })
  }

  @Builder
  private deviceCard(profile: Profile) {
    Column({ space: 13 }) {
      Row() {
        Text('▣').fontSize(28).fontColor('#77906E').width(48).height(48).textAlign(TextAlign.Center).backgroundColor('#EFF4E9').borderRadius(12)
        Blank()
        Text(this.sessionProfile?.id === profile.id && this.screen.approved ? (this.screen.rendered > 0 ? '● 正在查看' : '等待画面') : '已保存')
          .fontSize(11).fontColor('#778780')
        Button(this.favorites.includes(profile.id) ? '★' : '☆', { type: ButtonType.Normal })
          .fontSize(22).fontColor('#69815E').backgroundColor(Color.Transparent).width(44).height(44)
          .accessibilityText(this.favorites.includes(profile.id) ? `取消常用 ${profile.name}` : `设为常用 ${profile.name}`)
          .enabled(!this.saving).onClick(() => this.toggleFavorite(profile.id))
      }.width('100%')
      Text(profile.name).fontSize(17).fontWeight(FontWeight.Medium).fontColor('#20352F').maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
      Text(profile.target).fontSize(13).fontColor('#778780').maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
      Text(profile.mode === 'direct' ? (canConnectProfile(profile) ? 'IP 直连 · 现场批准后仅查看' : '请补充可信设备 ID 和公钥') : '设备 ID / 中继连接尚未实现')
        .fontSize(11).fontColor('#778780')
      Flex({ wrap: FlexWrap.Wrap }) {
        Button('连接', { type: ButtonType.Normal }).height(44).borderRadius(10).backgroundColor('#E5F2E9').fontColor('#196B52').fontSize(13)
          .margin({ right: 8, bottom: 4 }).enabled(canConnectProfile(profile) && !this.saving).onClick(() => this.connect(profile))
        Button('编辑', { type: ButtonType.Normal }).height(44).backgroundColor(Color.Transparent).fontColor('#53695C').fontSize(13)
          .enabled(!this.saving).onClick(() => this.openEditor(profile))
        Button('移除', { type: ButtonType.Normal }).height(44).backgroundColor(Color.Transparent).fontColor('#A8443C').fontSize(13)
          .enabled(!this.saving).onClick(() => { this.editingId = profile.id; this.name = profile.name; this.formError = ''; this.modal = 'delete'; })
      }.width('100%')
    }.padding(20).width('100%').alignItems(HorizontalAlign.Start).backgroundColor('#FFFFFF')
      .borderRadius(16).border({ width: 1, color: '#E4EAE5' })
  }

  @Builder
  private devicesPage() {
    GridRow({ columns: this.windowWidth < 600 ? 1 : 2, gutter: 20 }) {
      GridCol() { this.welcome() }
      GridCol() { this.connectionCard() }
    }.width('100%')
    Row({ space: 12 }) {
      Text('我的设备').fontSize(20).fontWeight(FontWeight.Medium).fontColor('#20352F')
      Text(String(this.profiles.length)).fontSize(12).fontColor('#778780')
      Blank()
      Button('导入配置', { type: ButtonType.Normal }).height(44).fontSize(13).fontColor('#196B52').backgroundColor(Color.Transparent)
        .enabled(!this.loading && !this.saving && this.storeError.length === 0).onClick(() => this.openImport())
    }.width('100%').margin({ top: 8 })
    TextInput({ text: this.search, placeholder: '搜索名称、IP 或设备 ID' }).height(44).width('100%').backgroundColor('#FFFFFF').borderRadius(10)
      .onChange((value: string) => { this.search = value; })
    Row({ space: 10 }) {
      Button('全部设备', { type: ButtonType.Normal }).height(44).fontSize(13).borderRadius(10)
        .backgroundColor(!this.onlyFavorites ? '#196B52' : '#F0F5EF').fontColor(!this.onlyFavorites ? '#FFFFFF' : '#196B52')
        .onClick(() => { this.onlyFavorites = false; })
      Button('常用', { type: ButtonType.Normal }).height(44).fontSize(13).borderRadius(10)
        .backgroundColor(this.onlyFavorites ? '#196B52' : '#F0F5EF').fontColor(this.onlyFavorites ? '#FFFFFF' : '#196B52')
        .onClick(() => { this.onlyFavorites = true; })
    }.width('100%')
    if (this.filteredProfiles().length === 0 && !this.loading) {
      Column({ space: 12 }) {
        Text(this.profiles.length === 0 ? '还没有保存的设备' : '没有匹配的设备').fontSize(17).fontColor('#53695C')
        Text(this.profiles.length === 0 ? '添加设备，或导入被控端的公开配置。' : '试试其他关键词，或切换到全部设备。').fontSize(13).fontColor('#778780')
      }.width('100%').padding(30).backgroundColor('#FFFFFF').borderRadius(16)
    }
    GridRow({ columns: this.windowWidth < 600 ? 1 : this.windowWidth < 1100 ? 2 : 3, gutter: 16 }) {
      ForEach(this.filteredProfiles(), (profile: Profile) => {
        GridCol() { this.deviceCard(profile) }
      }, (profile: Profile) => JSON.stringify(profile))
    }.width('100%')
    Text('设备在线状态将在连接时核实 · 保存设备不代表获得访问权限').fontSize(11).fontColor('#778780').width('100%')
  }

  @Builder
  private recentPage() {
    if (this.recent.length === 0) {
      Column({ space: 16 }) {
        Text('◷').fontSize(38).fontColor('#8DA184')
        Text('还没有连接记录').fontSize(19).fontColor('#20352F')
        Text('经 Windows 本机批准的连接会显示在这里。').fontSize(13).fontColor('#778780')
      }.width('100%').padding(36).backgroundColor('#FFFFFF').borderRadius(18)
    }
    ForEach(this.recent, (item: RecentConnection) => {
      Column({ space: 9 }) {
        Text(item.name).fontSize(17).fontWeight(FontWeight.Medium).fontColor('#20352F')
        Text('本机已批准查看 · IP 直连').fontSize(13).fontColor('#196B52')
        Text(`${item.target} · ${new Date(item.at).toLocaleString()}`).fontSize(12).fontColor('#778780')
        this.action('重新连接', () => {
          const profile = this.profiles.find((p: Profile) => p.id === item.profileId);
          if (profile) { this.connect(profile); } else { this.notice = '设备已移除，请重新添加可信资料。'; }
        }, false, () => this.profiles.some((p: Profile) => p.id === item.profileId && canConnectProfile(p)))
      }.width('100%').padding(22).alignItems(HorizontalAlign.Start).backgroundColor('#FFFFFF').borderRadius(16)
    }, (item: RecentConnection) => `${item.profileId}-${item.at}`)
    Text('记录只保存在本机，不包含密码、画面或输入内容。').fontSize(12).fontColor('#778780')
  }

  @Builder
  private setting(label: string, value: string) {
    Column({ space: 7 }) {
      Text(label).fontSize(14).fontColor('#53695C')
      Text(value).fontSize(13).fontColor('#778780').lineHeight(22)
    }.width('100%').alignItems(HorizontalAlign.Start).padding({ top: 14, bottom: 14 })
      .border({ width: { bottom: 1 }, color: '#EDF0EB' })
  }

  @Builder
  private settingsPage() {
    Column({ space: 4 }) {
      Text('连接服务').fontSize(19).fontWeight(FontWeight.Medium).fontColor('#20352F')
      this.setting('连接方式', 'IP 直连；设备 ID 与中继尚未实现')
      this.setting('访问方式', 'Windows 本机逐次批准；无人值守尚未实现')
      this.setting('身份与加密', '预存可信设备身份；签名验证与 v1 加密，失败即停止')
    }.width('100%').padding(24).alignItems(HorizontalAlign.Start).backgroundColor('#FFFFFF').borderRadius(18)
    Column({ space: 4 }) {
      Text('远程桌面').fontSize(19).fontWeight(FontWeight.Medium).fontColor('#20352F')
      this.setting('进入模式', '仅查看；系统键鼠与中文输入尚未开放')
      this.setting('画面显示', '保持比例，适应窗口；单主屏')
      this.setting('画面质量', 'VP8 软件视频 · 最大 1280 × 720 · 约 8 fps 上限；画质切换尚未实现')
      this.setting('文件、剪贴板与声音', '尚未实现，不随连接自动开放')
      this.setting('离开应用', '离开前台会断开；再次连接需要重新批准')
    }.width('100%').padding(24).alignItems(HorizontalAlign.Start).backgroundColor('#FFFFFF').borderRadius(18)
    Column({ space: 14 }) {
      Text('连接诊断').fontSize(18).fontColor('#20352F')
      Text('网络预检、单次登录验证、正式非媒体入口与独立 Windows DEMO。DEMO 输入只作用于演示窗口。')
        .fontSize(13).fontColor('#778780').lineHeight(22)
      this.action('打开连接诊断', () => this.openDiagnostics())
    }.width('100%').padding(24).alignItems(HorizontalAlign.Start).backgroundColor('#FFFFFF').borderRadius(18)
  }

  @Builder
  private workspace() {
    Column() {
      Row() {
        if (this.windowWidth >= 600) { this.sideNav() }
        Scroll() {
          Column({ space: 20 }) {
            Row() {
              Text('工作空间   ›   ' + (this.page === 'devices' ? '我的设备' : this.page === 'recent' ? '最近连接' : '偏好设置'))
                .fontSize(12).fontColor('#778780')
              Blank()
              Text('本机资料').fontSize(11).fontColor('#69815E').padding(6).backgroundColor('#EDF4E9').borderRadius(6)
            }.width('100%')
            if (this.sessionProfile && screenIsLive(this.screen)) {
              Button({ type: ButtonType.Normal }) {
                Row({ space: 10 }) {
                  Text(`${this.sessionProfile.name} · ${this.screen.title}`).fontSize(13).fontColor('#196B52').layoutWeight(1).maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
                  Text('返回会话 →').fontSize(13).fontColor('#196B52')
                }.width('100%').padding(12)
              }.width('100%').height(48).backgroundColor('#E5F2E9').borderRadius(10).onClick(() => { this.sessionOpen = true; })
            }
            Row({ space: 12 }) {
              Column({ space: 9 }) {
                Text(this.page === 'settings' ? 'MAKE YOURSELF AT HOME' : 'YOUR WORKSPACE, ANYWHERE').fontSize(10).letterSpacing(1.4).fontColor('#778780')
                Text(this.page === 'devices' ? '我的设备' : this.page === 'recent' ? '最近连接' : '偏好设置').fontSize(29).fontWeight(FontWeight.Bold).fontColor('#20352F')
                Text(this.page === 'devices' ? '让距离，留在屏幕之外。' : this.page === 'recent' ? '每一次相连，都有迹可循。' : '当前可用能力，清楚呈现。').fontSize(13).fontColor('#778780')
              }.alignItems(HorizontalAlign.Start).layoutWeight(1)
              if (this.page === 'devices') {
                Button(this.windowWidth < 600 ? '＋' : '＋ 添加设备', { type: ButtonType.Normal })
                  .height(44).fontSize(14).fontColor('#196B52').backgroundColor('#F0F5EF').borderRadius(10)
                  .accessibilityText('添加设备').enabled(!this.loading && !this.saving && this.storeError.length === 0)
                  .onClick(() => this.openEditor())
              }
            }.width('100%')
            if (this.loading) { Text('正在加载设备资料…').fontSize(14).fontColor('#778780') }
            if (this.storeError.length > 0) {
              Text(this.storeError).fontSize(13).fontColor('#A8443C').width('100%')
              this.action('重新加载', () => this.loadProfiles())
            }
            if (this.notice.length > 0) {
              Text(this.notice).fontSize(13).fontColor('#53695C').width('100%').padding(14).backgroundColor('#EAF1E5').borderRadius(10)
            }
            if (this.page === 'devices') { this.devicesPage() }
            else if (this.page === 'recent') { this.recentPage() }
            else { this.settingsPage() }
          }.width('100%').padding(this.windowWidth < 600 ? 20 : 28).alignItems(HorizontalAlign.Start)
        }.layoutWeight(1).height('100%').align(Alignment.TopStart).scrollBar(BarState.Auto)
      }.width('100%').layoutWeight(1)
      if (this.windowWidth < 600) {
        Row() {
          this.navItem('devices', $r('app.media.desk_devices'), '设备')
          this.navItem('recent', $r('app.media.desk_recent'), '最近连接')
          this.navItem('settings', $r('app.media.desk_session_settings'), '偏好设置')
        }.width('100%').justifyContent(FlexAlign.SpaceAround).height(68).padding(4).backgroundColor('#FFFFFF')
          .border({ width: { top: 1 }, color: '#E4EAE5' })
      }
    }.width('100%').height('100%').backgroundColor('#F6F8F5')
  }

  @Builder
  private field(label: string, value: string, placeholder: string, change: (value: string) => void) {
    Column({ space: 7 }) {
      Text(label).fontSize(13).fontColor('#53695C')
      TextInput({ text: value, placeholder: placeholder }).height(46).width('100%').fontSize(14).backgroundColor('#F6F8F5')
        .borderRadius(10).onChange(change)
    }.width('100%').alignItems(HorizontalAlign.Start)
  }

  @Builder
  private editor() {
    Column({ space: 16 }) {
      if (this.modal === 'edit') {
        this.field('设备名称', this.name, '例如：工作室电脑', (v: string) => { this.name = v; })
        Text('IP 直连 · 现场批准后仅查看').fontSize(12).fontColor('#196B52')
        this.field('IP 地址', this.address, '192.168.1.10 或 ::1', (v: string) => { this.address = v; })
        this.field('端口', this.port, '21120', (v: string) => { this.port = v; })
        this.field('被控设备 ID', this.peerId, '6 至 20 位数字', (v: string) => { this.peerId = v; })
        this.field('可信 Ed25519 公钥', this.peerPublicKey, '从被控端现场核对并复制', (v: string) => { this.peerPublicKey = v; })
        this.field('SHA-256 指纹（可选）', this.fingerprint, '64 位十六进制字符', (v: string) => { this.fingerprint = v; })
        Text('请从可信来源取得并核对 ID / 公钥。保存资料不会自动连接或授予访问权限。').fontSize(12).fontColor('#778780').lineHeight(20)
        this.action('保存设备', () => this.saveForm(), true, () => !this.saving)
      } else if (this.modal === 'import') {
        Text('粘贴被控端导出的公开配置，预览并核对后保存。不要粘贴密码或私钥。').fontSize(13).fontColor('#778780').lineHeight(22)
        TextArea({ text: this.importJson, placeholder: '公开 profile JSON' }).height(170).width('100%').fontSize(13).backgroundColor('#F6F8F5')
          .onChange((value: string) => { this.importJson = value; this.importPreview = undefined; this.formError = ''; })
        this.action('验证并预览', () => this.previewImport(), false, () => !this.saving && this.importJson.length > 0)
        if (this.importPreview) {
          Text(this.importPreview.name).fontSize(17).fontColor('#20352F')
          Text(`地址：${this.importPreview.target}\n设备 ID：${this.importPreview.peerId ?? ''}`).fontSize(13).fontColor('#53695C').lineHeight(24)
          Text(`可信公钥：${this.importPreview.peerPublicKey ?? ''}\n指纹：${this.importPreview.peerFingerprint}`).fontSize(12).fontColor('#778780').lineHeight(22)
          this.action('确认保存', () => this.saveImport(), true, () => !this.saving)
        }
      } else {
        Text(`从本机移除「${this.name}」？`).fontSize(18).fontColor('#20352F')
        Text('这不会修改 Windows 被控端。之后需要重新添加可信设备资料才能连接。').fontSize(13).fontColor('#778780').lineHeight(22)
        this.action('确认移除', () => this.deleteDevice(), false, () => !this.saving)
      }
      if (this.formError.length > 0) { Text(this.formError).fontSize(13).fontColor('#A8443C').width('100%') }
    }.width('100%').alignItems(HorizontalAlign.Start)
  }

  build() {
    Stack() {
      if (this.sessionProfile) {
        ForEach([this.sessionKey], (_key: number) => {
          RemoteSession({ screen: this.screen, deviceName: this.sessionProfile?.name ?? '', endpoint: this.sessionProfile?.target ?? '',
            surfaceActive: screenIsLive(this.screen), controller: this.surfaceController,
            onBack: () => { this.sessionOpen = false; }, onDisconnect: () => this.disconnect(),
            onRetry: () => { if (this.sessionProfile) { this.connect(this.sessionProfile); } } })
        }, (key: number) => String(key))
      }
      if (!this.sessionOpen) { this.workspace() }
      if (this.modal.length > 0) {
        Column().width('100%').height('100%').backgroundColor('#8020382E')
        Column({ space: 16 }) {
          Row() {
            Text(this.modal === 'edit' ? (this.editingId ? '编辑设备' : '添加设备') : this.modal === 'import' ? '导入公开配置' : '移除设备')
              .fontSize(21).fontWeight(FontWeight.Medium).fontColor('#20352F')
            Blank()
            Button('关闭', { type: ButtonType.Normal }).height(44).fontSize(13).fontColor('#778780').backgroundColor(Color.Transparent)
              .enabled(!this.saving).onClick(() => { this.modal = ''; this.importJson = ''; this.importPreview = undefined; })
          }.width('100%')
          Scroll() { this.editor() }.width('100%').layoutWeight(1).scrollBar(BarState.Auto)
        }.width('92%').constraintSize({ maxWidth: 600 }).height('90%').padding(24).backgroundColor('#FFFFFF').borderRadius(20)
      }
    }.width('100%').height('100%').backgroundColor('#F6F8F5')
      .onAreaChange((_old: Area, area: Area) => { this.windowWidth = Number(area.width); })
  }
}
