import { ScreenState, connectionPathLabel } from '../model/DeskModels';
import { KeyCode } from '@kit.InputKit';
import { SystemInputCommand, normalizePointerDelta, canRequestSystemInput, canSendSystemInput, canSubmitInputText } from '../model/InputModels';

@Component
export struct RemoteSession {
  @Prop @Watch('inputAvailabilityChanged') screen: ScreenState = new ScreenState();
  @Prop deviceName: string = '';
  @Prop endpoint: string = '';
  @Prop accessMode: string = '';
  @Prop surfaceActive: boolean = false;
  @Prop @Watch('inputAvailabilityChanged') sessionVisible: boolean = true;
  controller: XComponentController = new XComponentController();
  onBack: () => void = () => {};
  onDisconnect: () => void = () => {};
  onRetry: () => void = () => {};
  onInput: (command: string) => number = () => 1;
  onReleaseInput: () => void = () => {};
  onSetInputEnabled: (enabled: boolean) => number = () => 1;
  @State private areaWidth: number = 390;
  @State private areaHeight: number = 720;
  @State private toolbarCollapsed: boolean = false;
  @State private toolsOpen: boolean = false;
  @State private controlMode: boolean = false;
  @State private inputPanel: string = '';
  @State private leftHeld: boolean = false;
  @State private inputNotice: string = '';
  @State private inputText: string = '';
  @State private previewActive: boolean = false;
  private textController: TextInputController = new TextInputController();
  private touchId: number = -1;
  private touchX: number = 0;
  private touchY: number = 0;
  private touchDistance: number = 0;
  private touchStarted: number = 0;
  private pendingDx: number = 0;
  private pendingDy: number = 0;
  private lastMoveAt: number = 0;
  private heldKeys: Set<number> = new Set<number>();

  aboutToDisappear(): void { this.stopInput(); }

  private readyForInput(): boolean {
    return canSendSystemInput(this.screen, this.sessionVisible, this.controlMode);
  }

  private inputAvailabilityChanged(): void {
    if (!canRequestSystemInput(this.screen, this.sessionVisible)) { this.stopInput(false); }
    else if (this.screen.code === 'INPUT_STATE' && !this.screen.inputGranted && this.controlMode) {
      this.stopInput(false);
      this.inputNotice = '键鼠未启用或已关闭，当前仅查看。';
    }
  }

  private releasePressed(force: boolean = false, notifyServer: boolean = true): void {
    const release = force || this.leftHeld || this.heldKeys.size > 0;
    this.leftHeld = false;
    this.heldKeys.clear();
    this.touchId = -1;
    this.pendingDx = 0;
    this.pendingDy = 0;
    if (release && notifyServer) { this.onReleaseInput(); }
  }

  private flushPointer(): boolean {
    if (this.pendingDx === 0 && this.pendingDy === 0) { return true; }
    const dx = this.pendingDx;
    const dy = this.pendingDy;
    this.pendingDx = 0;
    this.pendingDy = 0;
    this.lastMoveAt = Date.now();
    return this.send({ kind: 'move_relative', dx: dx, dy: dy });
  }

  private inputBlur(): void {
    if (!this.controlMode) { this.releasePressed(false, false); return; }
    this.flushPointer();
    this.releasePressed(true);
  }

  private stopInput(notifyServer: boolean = true): void {
    const shouldDisable = this.controlMode || this.screen.inputGranted || this.screen.code === 'INPUT_ENABLING';
    const wasEditing = this.inputPanel === 'keyboard';
    this.controlMode = false;
    this.releasePressed(false, false);
    this.inputPanel = '';
    this.inputText = '';
    this.previewActive = false;
    this.inputNotice = '';
    if (notifyServer && shouldDisable && this.onSetInputEnabled(false) !== 0) {
      this.inputNotice = '无法确认键鼠关闭，正在结束连接。';
    }
    if (wasEditing) { this.textController.stopEditing(); }
  }

  private openInput(panel: string): void {
    if (!canRequestSystemInput(this.screen, this.sessionVisible)) { return; }
    if (this.controlMode && (this.screen.inputGranted || this.screen.code === 'INPUT_ENABLING')) {
      this.releasePressed();
      const wasEditing = this.inputPanel === 'keyboard';
      this.inputPanel = panel;
      this.inputText = '';
      this.previewActive = false;
      if (wasEditing) { this.textController.stopEditing(); }
    } else {
      this.stopInput(false);
      if (this.onSetInputEnabled(true) !== 0) {
        this.inputNotice = '键鼠开启请求未能提交，请检查连接与设备能力。';
        return;
      }
      this.controlMode = true;
      this.inputPanel = panel;
    }
    this.inputNotice = '';
    this.toolsOpen = false;
  }

  private inputStatus(): string {
    if (this.accessMode) { return '只读查看 · 键鼠不可用'; }
    if (this.screen.code === 'INPUT_ENABLING') { return '正在启用键鼠 · 等待设备确认'; }
    if (this.screen.code === 'INPUT_DISABLING') { return '正在关闭键鼠 · 本地输入已停止'; }
    if (this.readyForInput()) { return '控制中 · 键鼠已开启'; }
    return this.screen.inputSupported ? '仅查看 · 点“控制”开始操作' : '仅查看 · 当前设备未开放键鼠能力';
  }

  private send(command: SystemInputCommand): boolean {
    if (!this.readyForInput()) { this.stopInput(); return false; }
    if (command.kind !== 'move_relative' && !this.flushPointer()) { return false; }
    const status = this.onInput(JSON.stringify(command));
    if (status !== 0) {
      this.stopInput();
      this.inputNotice = status === 2 ? '键鼠已关闭或不可用，当前仅查看。' : '输入未能排队，已暂停控制。请检查连接后重试。';
      return false;
    }
    return true;
  }

  private clickMouse(button: string): void {
    if (this.leftHeld) { this.releasePressed(true); }
    if (this.send({ kind: 'button', button: button, down: true })) {
      this.send({ kind: 'button', button: button, down: false });
    }
  }

  private toggleLeftHold(): void {
    const next = !this.leftHeld;
    if (this.send({ kind: 'button', button: 'left', down: next })) { this.leftHeld = next; }
  }

  private tapKey(code: string): void {
    if (this.send({ kind: 'key', code: code, down: true })) {
      this.send({ kind: 'key', code: code, down: false });
    }
  }

  private submitText(): void {
    if (!this.readyForInput() || !canSubmitInputText(this.inputText, this.previewActive)) { return; }
    if (this.send({ kind: 'text', text: this.inputText })) {
      this.inputText = '';
      this.inputNotice = '文本已排队，请以远端画面为准。';
    }
  }

  private pointerTouch(event: TouchEvent): void {
    event.stopPropagation();
    if (event.type === TouchType.Cancel || !this.readyForInput()) { this.stopInput(); return; }
    if (event.changedTouches.length === 0) { return; }
    if (event.type === TouchType.Down) {
      if (this.touchId >= 0 || event.touches.length > 1) { this.releasePressed(true); return; }
      const point = event.changedTouches[0];
      this.touchId = point.id;
      this.touchX = point.x;
      this.touchY = point.y;
      this.touchDistance = 0;
      this.touchStarted = Date.now();
      try { this.getUIContext().getFocusController().requestFocus('system-touchpad'); }
      catch { this.inputNotice = '可点击触控板重新获取物理键盘焦点。'; }
      return;
    }
    for (const point of event.changedTouches) {
      if (point.id !== this.touchId) { continue; }
      if (event.type === TouchType.Move) {
        const dx = point.x - this.touchX;
        const dy = point.y - this.touchY;
        this.touchDistance += Math.abs(dx) + Math.abs(dy);
        this.touchX = point.x;
        this.touchY = point.y;
        const delta = normalizePointerDelta(dx, dy, this.surfaceWidth(), this.surfaceHeight());
        this.pendingDx = Math.max(-65535, Math.min(65535, this.pendingDx + delta.dx));
        this.pendingDy = Math.max(-65535, Math.min(65535, this.pendingDy + delta.dy));
        if (Date.now() - this.lastMoveAt >= 20) { this.flushPointer(); }
      } else if (event.type === TouchType.Up) {
        const dx = point.x - this.touchX;
        const dy = point.y - this.touchY;
        this.touchDistance += Math.abs(dx) + Math.abs(dy);
        const delta = normalizePointerDelta(dx, dy, this.surfaceWidth(), this.surfaceHeight());
        this.pendingDx = Math.max(-65535, Math.min(65535, this.pendingDx + delta.dx));
        this.pendingDy = Math.max(-65535, Math.min(65535, this.pendingDy + delta.dy));
        this.touchId = -1;
        if (!this.flushPointer()) { return; }
        if (this.touchDistance < 6 && Date.now() - this.touchStarted < 500 && !this.leftHeld) { this.clickMouse('left'); }
      }
    }
  }

  private hardwareCode(code: number): string {
    if (code >= KeyCode.KEYCODE_A && code <= KeyCode.KEYCODE_Z) { return 'Key' + String.fromCharCode(65 + code - KeyCode.KEYCODE_A); }
    if (code >= KeyCode.KEYCODE_0 && code <= KeyCode.KEYCODE_9) { return 'Digit' + String(code - KeyCode.KEYCODE_0); }
    switch (code) {
      case KeyCode.KEYCODE_ENTER: return 'Enter';
      case KeyCode.KEYCODE_ESCAPE: return 'Escape';
      case KeyCode.KEYCODE_TAB: return 'Tab';
      case KeyCode.KEYCODE_DEL: return 'Backspace';
      case KeyCode.KEYCODE_FORWARD_DEL: return 'Delete';
      case KeyCode.KEYCODE_SPACE: return 'Space';
      case KeyCode.KEYCODE_DPAD_LEFT: return 'ArrowLeft';
      case KeyCode.KEYCODE_DPAD_RIGHT: return 'ArrowRight';
      case KeyCode.KEYCODE_DPAD_UP: return 'ArrowUp';
      case KeyCode.KEYCODE_DPAD_DOWN: return 'ArrowDown';
      case KeyCode.KEYCODE_MOVE_HOME: return 'Home';
      case KeyCode.KEYCODE_MOVE_END: return 'End';
      case KeyCode.KEYCODE_PAGE_UP: return 'PageUp';
      case KeyCode.KEYCODE_PAGE_DOWN: return 'PageDown';
      case KeyCode.KEYCODE_SHIFT_LEFT:
      case KeyCode.KEYCODE_SHIFT_RIGHT: return 'Shift';
      case KeyCode.KEYCODE_CTRL_LEFT:
      case KeyCode.KEYCODE_CTRL_RIGHT: return 'Control';
      case KeyCode.KEYCODE_ALT_LEFT:
      case KeyCode.KEYCODE_ALT_RIGHT: return 'Alt';
      default: return '';
    }
  }

  private hardwareKey(event: KeyEvent): boolean {
    if (!this.readyForInput() || this.inputPanel !== 'pointer') { return false; }
    const code = this.hardwareCode(event.keyCode);
    if (code.length === 0) { return false; }
    if (event.type === KeyType.Down) {
      if (this.send({ kind: 'key', code: code, down: true })) { this.heldKeys.add(event.keyCode); }
    } else if (event.type === KeyType.Up) {
      if (!this.heldKeys.delete(event.keyCode)) { return true; }
      let sharedKeyHeld = false;
      this.heldKeys.forEach((key: number) => { if (this.hardwareCode(key) === code) { sharedKeyHeld = true; } });
      if (!sharedKeyHeld) { this.send({ kind: 'key', code: code, down: false }); }
    }
    return true;
  }

  private toggleTools(): void {
    this.stopInput();
    this.toolsOpen = !this.toolsOpen;
  }

  private compact(): boolean { return this.areaWidth < 600; }

  private terminal(): boolean {
    return this.screen.phase === 'ended' || this.screen.phase === 'failed' || this.screen.phase === 'idle';
  }

  private pictureVisible(): boolean {
    return this.surfaceActive && this.screen.approved && this.screen.rendered > 0 &&
      (this.screen.phase === 'viewing' || this.screen.phase === 'approved');
  }

  private ratio(): number {
    return this.screen.width > 0 && this.screen.height > 0 ? this.screen.width / this.screen.height : 16 / 9;
  }

  private surfaceWidth(): number { return Math.min(this.areaWidth, this.areaHeight * this.ratio()); }
  private surfaceHeight(): number { return this.surfaceWidth() / this.ratio(); }
  private panelTop(): number { return this.compact() ? 134 : 80; }
  private inputPanelTop(): number {
    return this.inputPanel === 'keyboard' ? Math.min(this.panelTop(), Math.max(0, this.areaHeight - 300)) : this.panelTop();
  }

  private leave(): void {
    this.stopInput();
    this.toolsOpen = false;
    this.onBack();
  }

  private disconnect(): void {
    this.stopInput();
    this.toolsOpen = false;
    this.toolbarCollapsed = false;
    this.onDisconnect();
  }

  private retry(): void {
    this.stopInput();
    this.toolsOpen = false;
    this.toolbarCollapsed = false;
    this.onRetry();
  }

  @Builder
  private iconButton(icon: Resource, label: string, action: () => void) {
    Button({ type: ButtonType.Normal }) {
      Image(icon).width(20).height(20)
    }.width(44).height(44).borderRadius(9).backgroundColor(Color.Transparent)
      .accessibilityText(label).onClick(action)
  }

  @Builder
  private identity() {
    Column({ space: 3 }) {
      Text(this.deviceName || '远程设备').fontSize(13).fontWeight(FontWeight.Medium)
        .fontColor('#20352F').maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
      Text('现场批准 · IP 直连').fontSize(10).fontColor('#778780').maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }.alignItems(HorizontalAlign.Start).layoutWeight(1)
  }

  @Builder
  private modeSwitch() {
    Row({ space: 3 }) {
      Button('仅查看', { type: ButtonType.Normal }).fontSize(12).height(38).fontColor('#196B52')
        .padding({ left: 9, right: 9 }).backgroundColor(this.controlMode ? Color.Transparent : '#FFFFFF').borderRadius(6)
        .onClick(() => this.stopInput())
      Button(this.screen.code === 'INPUT_ENABLING' ? '开启中' : '控制', { type: ButtonType.Normal }).fontSize(12).height(38)
        .padding({ left: 9, right: 9 }).borderRadius(6).backgroundColor(this.controlMode ? '#FFFFFF' : Color.Transparent)
        .fontColor('#196B52').enabled(canRequestSystemInput(this.screen, this.sessionVisible))
        .onClick(() => this.openInput('pointer'))
    }.padding(3).borderRadius(9).backgroundColor('#EEF3E9').border({ width: 1, color: '#DCE6D7' })
  }

  @Builder
  private unavailableTool(icon: Resource, label: string) {
    Button({ type: ButtonType.Normal }) {
      Column({ space: 2 }) {
        Image(icon).width(18).height(18)
        Text(label).fontSize(10).fontColor('#627662')
      }
    }.width(44).height(44).borderRadius(8).backgroundColor(Color.Transparent)
      .enabled(false).accessibilityText(`${label}，尚未实现`)
  }

  @Builder
  private tools() {
    Row({ space: 1 }) {
      this.unavailableTool($r('app.media.desk_session_folder'), '文件')
      Button({ type: ButtonType.Normal }) {
        Column({ space: 2 }) {
          Image($r('app.media.desk_session_keyboard')).width(18).height(18)
          Text('键盘').fontSize(10).fontColor('#426A4D')
        }
      }.width(44).height(44).borderRadius(8).backgroundColor(this.inputPanel === 'keyboard' ? '#E4EFDF' : Color.Transparent)
        .enabled(canRequestSystemInput(this.screen, this.sessionVisible)).accessibilityText('键盘与确认文本')
        .onClick(() => this.openInput('keyboard'))
      Button({ type: ButtonType.Normal }) {
        Column({ space: 2 }) {
          Image($r('app.media.desk_session_settings')).width(18).height(18)
          Text('工具').fontSize(10).fontColor('#426A4D')
        }
      }.width(44).height(44).borderRadius(8)
        .backgroundColor(this.toolsOpen ? '#E4EFDF' : Color.Transparent)
        .accessibilityText(this.toolsOpen ? '关闭会话工具' : '打开会话工具')
        .onClick(() => this.toggleTools())
    }
  }

  @Builder
  private endButtons() {
    Row({ space: 1 }) {
      Button('断开', { type: ButtonType.Normal }).height(44).fontSize(12).fontColor('#A8443C')
        .padding({ left: 12, right: 12 }).borderRadius(8).backgroundColor('#FFF5F2')
        .onClick(() => this.disconnect())
      this.iconButton($r('app.media.desk_session_up'), '收起工具栏', () => {
        this.stopInput();
        this.toolsOpen = false;
        this.toolbarCollapsed = true;
      })
    }
  }

  @Builder
  private sessionToolbar() {
    Column({ space: 7 }) {
      if (this.compact()) {
        Row({ space: 3 }) {
          this.iconButton($r('app.media.desk_session_back'), '返回设备，保持连接', () => this.leave())
          this.identity()
          this.endButtons()
        }.width('100%')
        Row() {
          this.modeSwitch()
          Blank()
          this.tools()
        }.width('100%')
      } else {
        Row({ space: 6 }) {
          this.iconButton($r('app.media.desk_session_back'), '返回设备，保持连接', () => this.leave())
          this.identity()
          this.modeSwitch()
          this.tools()
          this.endButtons()
        }.width('100%')
      }
    }.padding(8).width(Math.min(this.areaWidth - 24, 920)).borderRadius(14)
      .backgroundColor('#F9FCF7').border({ width: 1, color: '#DCE6DC' })
      .position({ x: (this.areaWidth - Math.min(this.areaWidth - 24, 920)) / 2, y: 12 })
  }

  @Builder
  private collapsedToolbar() {
    Button({ type: ButtonType.Normal }) {
      Row({ space: 9 }) {
        Image($r('app.media.desk_session_monitor')).width(18).height(18)
        Text(this.deviceName || '远程设备').fontSize(12).fontColor('#20352F')
          .maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis }).layoutWeight(1)
        Text(this.readyForInput() ? '控制中' : this.screen.code === 'INPUT_DISABLING' ? '关闭中' : '仅查看').fontSize(10).fontColor('#426A4D').padding(5).backgroundColor('#E3EFDB').borderRadius(5)
        Image($r('app.media.desk_session_down')).width(16).height(16)
      }.width('100%').padding({ left: 14, right: 14, top: 12, bottom: 12 })
    }.width(Math.min(this.areaWidth - 24, 360)).borderRadius({ bottomLeft: 12, bottomRight: 12 })
      .backgroundColor('#F9FCF7')
      .position({ x: (this.areaWidth - Math.min(this.areaWidth - 24, 360)) / 2, y: 0 })
      .accessibilityText('展开会话工具栏，包含断开按钮')
      .onClick(() => { this.toolbarCollapsed = false; })
  }

  @Builder
  private stat(label: string, value: () => string) {
    Column({ space: 6 }) {
      Text(label).fontSize(11).fontColor('#778780')
      Text(value()).fontSize(13).fontColor('#20352F').width('100%')
    }.alignItems(HorizontalAlign.Start).width('100%').padding({ top: 12, bottom: 12 })
      .border({ width: { bottom: 1 }, color: '#E4EAE5' })
  }

  @Builder
  private toolsPanel() {
    Column() {
      Row() {
        Text('会话工具').fontSize(16).fontWeight(FontWeight.Medium).fontColor('#20352F').layoutWeight(1)
        this.iconButton($r('app.media.desk_session_close'), '关闭会话工具', () => { this.stopInput(); this.toolsOpen = false; })
      }.padding({ left: 18, right: 5 }).width('100%')
      Scroll() {
        Column({ space: 12 }) {
          Column({ space: 7 }) {
            Text(this.inputStatus()).fontSize(15).fontWeight(FontWeight.Medium).fontColor('#196B52')
            Text(this.screen.inputSupported ? '接入已获批准，可在这里开关键鼠。关闭输入面板会请求关闭键鼠。声音、文件与剪贴板尚未实现。' : '当前被控入口未开放键鼠能力。声音、文件与剪贴板尚未实现。')
              .fontSize(12).fontColor('#778780').lineHeight(20)
          }.alignItems(HorizontalAlign.Start).width('100%').padding(14).borderRadius(10).backgroundColor('#EDF4E6')
          this.stat('连接地址', () => this.endpoint || '未提供')
          this.stat('实际连接方式', () => connectionPathLabel(this.screen.connectionPath))
          this.stat('设备身份', () => this.screen.verified ? '已通过可信身份核验' : '尚未完成核验')
          this.stat('本次授权', () => !this.screen.approved ? '尚未确认' :
            this.screen.accessMode === 'unattended' ? '无人值守凭据已验证 · 只读' :
              this.screen.accessMode === 'pair' ? 'Windows 已批准登记 · 只读' : 'Windows 本机已批准查看')
          this.stat('键盘与鼠标', () => this.screen.code === 'INPUT_ENABLING' ? '正在开启，等待状态回执' : this.screen.code === 'INPUT_DISABLING' ? '正在关闭，等待状态回执' : this.screen.inputGranted ? '设备已确认开启' : '设备当前未开启')
          Button('打开触控板', { type: ButtonType.Normal }).height(44).width('100%').fontColor('#FFFFFF').backgroundColor('#196B52').borderRadius(10)
            .enabled(canRequestSystemInput(this.screen, this.sessionVisible)).onClick(() => this.openInput('pointer'))
          this.stat('远端画面', () => this.screen.approved && this.screen.width > 0 && this.screen.height > 0 ?
            `${this.screen.width} × ${this.screen.height}` : '等待画面协商')
          this.stat('实际接收 / 呈现', () => `${this.screen.frames} 帧 / ${this.screen.rendered} 帧`)
          this.stat('已接收视频数据', () => `${Math.round(this.screen.bytes / 1024)} KiB`)
          this.stat('画面显示', () => '适应窗口 · 保持远端比例')
          this.stat('画质', () => 'VP8 软件视频 · 画质切换尚未实现')
          Text(this.accessMode ? '此连接仅可查看。Windows 可随时撤销已登记设备。' :
            '本次批准仅用于当前连接。断开或重新连接后，需要再次在 Windows 本机批准。')
            .fontSize(12).fontColor('#778780').lineHeight(20).width('100%')
        }.width('100%').padding({ left: 18, right: 18, top: 8, bottom: 20 })
      }.width('100%').layoutWeight(1).scrollBar(BarState.Auto)
    }.width(this.compact() ? this.areaWidth - 24 : 316)
      .height(Math.max(100, this.areaHeight - this.panelTop() - 12))
      .borderRadius(15).clip(true).backgroundColor('#FCFEFA').border({ width: 1, color: '#DCE6DC' })
      .position({ x: this.areaWidth - (this.compact() ? this.areaWidth - 24 : 316) - 12, y: this.panelTop() })
  }

  @Builder
  private inputAction(label: () => string, action: () => void, allowed: () => boolean = () => true) {
    Button(label(), { type: ButtonType.Normal }).height(44).fontSize(12).fontColor('#196B52')
      .padding({ left: 12, right: 12 }).borderRadius(8).backgroundColor('#EDF4E6')
      .margin({ right: 7, bottom: 7 }).enabled(this.readyForInput() && allowed()).focusOnTouch(false).onClick(action)
  }

  @Builder
  private pointerPad() {
    Stack() {
      Text(this.leftHeld ? '左键保持按下 · 滑动拖动\n再次点击按住按钮即可释放' : '单指滑动移动 · 轻点左击\n光标以 Windows 画面为准')
        .fontSize(12).fontColor('#627A55').textAlign(TextAlign.Center).lineHeight(22)
    }.width('100%').height(this.areaHeight < 480 ? 90 : 142).backgroundColor('#F0F5EB').borderRadius(12)
      .border({ width: 1, color: '#C9D9BC', style: BorderStyle.Dashed }).id('system-touchpad').focusable(true)
      .onTouch((event: TouchEvent) => this.pointerTouch(event))
      .onKeyEvent((event: KeyEvent) => this.hardwareKey(event)).onBlur(() => this.inputBlur())
  }

  @Builder
  private inputControls() {
    Column() {
      Row() {
        Text(this.inputPanel === 'pointer' ? '触控板 · 控制中' : '键盘与文字').fontSize(15).fontWeight(FontWeight.Medium).fontColor('#20352F').layoutWeight(1)
        this.iconButton($r('app.media.desk_session_close'), '关闭输入并切回仅查看', () => this.stopInput())
      }.width('100%').padding({ left: 16, right: 5 })
      if (this.inputPanel === 'pointer') {
        Column() { this.pointerPad() }.width('100%').padding({ left: 16, right: 16, top: 8, bottom: 8 })
      }
      Scroll() {
        Column({ space: 12 }) {
          if (this.inputPanel === 'pointer') {
            Flex({ wrap: FlexWrap.Wrap }) {
              this.inputAction(() => '左键', () => this.clickMouse('left'))
              this.inputAction(() => '右键', () => this.clickMouse('right'))
              this.inputAction(() => '中键', () => this.clickMouse('middle'))
              this.inputAction(() => this.leftHeld ? '释放左键' : '按住左键', () => this.toggleLeftHold())
              this.inputAction(() => '滚轮 ↑', () => { this.send({ kind: 'wheel', dx: 0, dy: 1 }); })
              this.inputAction(() => '滚轮 ↓', () => { this.send({ kind: 'wheel', dx: 0, dy: -1 }); })
              this.inputAction(() => '横滚 ←', () => { this.send({ kind: 'wheel', dx: -1, dy: 0 }); })
              this.inputAction(() => '横滚 →', () => { this.send({ kind: 'wheel', dx: 1, dy: 0 }); })
              this.inputAction(() => '释放全部按键', () => this.releasePressed(true))
            }.width('100%')
            Text('点击触控板后可使用外接物理键盘。文本选词请使用工具栏的键盘入口。').fontSize(11).fontColor('#778780').lineHeight(18)
          } else {
            Text('只发送确认后的文字。中文选词完成后点击发送，最多 512 UTF-8 字节。').fontSize(12).fontColor('#778780').lineHeight(20)
            TextInput({ text: this.inputText, controller: this.textController, placeholder: '输入并确认要发到 Windows 的文字' })
              .width('100%').height(48).fontSize(14).maxLength(512).enablePreviewText(true).backgroundColor('#F0F5EB')
              .onChange((value: string, preview?: PreviewText) => { this.inputText = value; this.previewActive = (preview?.value.length ?? 0) > 0; })
              .onBlur(() => this.inputBlur())
              .onSubmit((_type: EnterKeyType, event?: SubmitEvent) => { if (this.readyForInput()) { event?.keepEditableState(); this.submitText(); } })
            Button('发送确认文本', { type: ButtonType.Normal }).height(44).width('100%').fontSize(13).backgroundColor('#196B52').fontColor('#FFFFFF').borderRadius(9)
              .enabled(this.readyForInput() && canSubmitInputText(this.inputText, this.previewActive)).focusOnTouch(false).onClick(() => this.submitText())
            Flex({ wrap: FlexWrap.Wrap }) {
              ForEach(['Escape', 'Tab', 'Enter', 'Backspace', 'Delete', 'Space', 'ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End', 'PageUp', 'PageDown'], (code: string) => {
                this.inputAction(() => code, () => this.tapKey(code), () => !this.previewActive)
              }, (code: string) => code)
            }.width('100%')
          }
          if (this.inputNotice.length > 0) { Text(this.inputNotice).fontSize(11).fontColor('#778780').lineHeight(18).width('100%') }
        }.width('100%').padding({ left: 16, right: 16, top: 8, bottom: 16 })
      }.width('100%').layoutWeight(1).scrollBar(BarState.Auto)
    }.width(this.compact() ? this.areaWidth - 24 : 390)
      .height(Math.min(440, Math.max(80, this.areaHeight - this.inputPanelTop() - 12)))
      .backgroundColor('#FCFEFA').borderRadius(15).clip(true).border({ width: 1, color: '#DCE6DC' })
      .position({ x: this.areaWidth - (this.compact() ? this.areaWidth - 24 : 390) - 12, y: this.inputPanelTop() })
  }

  @Builder
  private progressStep(label: string, detail: () => string, complete: () => boolean, current: () => boolean) {
    Row({ space: 12 }) {
      Text(complete() ? '✓' : current() ? '···' : '○').fontSize(16)
        .fontColor(complete() || current() ? '#196B52' : '#9DAA9F')
        .width(30).height(30).textAlign(TextAlign.Center)
        .backgroundColor(complete() || current() ? '#E5F2E9' : '#F3F5F1').borderRadius(15)
      Column({ space: 4 }) {
        Text(label).fontSize(13).fontColor(current() || complete() ? '#20352F' : '#778780')
        Text(detail()).fontSize(11).fontColor('#778780').lineHeight(17)
      }.alignItems(HorizontalAlign.Start).layoutWeight(1)
    }.width('100%').alignItems(VerticalAlign.Center)
  }

  @Builder
  private stateCard() {
    Column() {
      Scroll() {
        Column({ space: this.areaHeight < 480 ? 12 : 18 }) {
          if (this.areaHeight >= 480) {
            Stack() {
              Image($r('app.media.desk_session_monitor')).width(28).height(28)
            }.width(60).height(60).borderRadius(20).backgroundColor('#E5F2E9')
            Text(this.terminal() ? 'REMOTE DESK' : this.accessMode === 'unattended' ? 'VERIFYING ACCESS' : 'WAITING FOR CONFIRMATION')
              .fontSize(10).fontColor('#778780').letterSpacing(1)
          }
          Text(this.screen.title || (this.terminal() ? '本次连接已结束' : '正在连接远程设备'))
            .fontSize(this.compact() ? 23 : 26).fontWeight(FontWeight.Medium).fontColor('#20352F').textAlign(TextAlign.Center)
          Text(this.deviceName || '远程设备').fontSize(14).fontColor('#196B52').textAlign(TextAlign.Center)
          Text(this.screen.detail || (this.terminal() ? '远程画面已停止，当前没有有效授权。' :
            '请等待 Windows 本机批准查看。批准前不会显示远程画面。'))
            .fontSize(13).fontColor('#778780').lineHeight(22).textAlign(TextAlign.Center).width('100%')
          if (!this.terminal()) {
            Column({ space: 20 }) {
              this.progressStep('连接远程设备', () => 'IP 直连', () => this.screen.phase !== 'connecting',
                () => this.screen.phase === 'connecting')
              this.progressStep('验证身份与加密通道', () => this.screen.phase === 'verifying' && this.screen.verified ?
                '身份已核验，正在建立加密通道' : this.screen.verified ? '可信身份与加密通道已核验' : '验证中，尚未显示画面',
                () => this.screen.phase === 'awaiting' || this.screen.phase === 'approved' || this.screen.phase === 'viewing',
                () => this.screen.phase === 'verifying')
              this.progressStep(this.accessMode === 'unattended' ? '验证无人值守凭据' :
                this.accessMode === 'pair' ? '等待 Windows 批准登记' : '等待本机批准查看',
                () => this.screen.approved ? '已验证，正在等待首帧呈现' : this.accessMode === 'unattended' ?
                  '正在核对本机凭据' : this.accessMode === 'pair' ? '请在 Windows 明确批准并登记只读访问' : '请在 Windows 连接管理器中接受请求',
                () => this.screen.approved, () => this.screen.phase === 'awaiting')
            }.width('100%').padding(20).borderRadius(14).backgroundColor('#FFFFFF')
              .border({ width: 1, color: '#E4EAE5' })
          }
        }.width('100%').padding({ bottom: 18 })
      }.width('100%').layoutWeight(1).scrollBar(BarState.Auto)
      Column({ space: 8 }) {
        Row({ space: 10 }) {
          Button('返回设备', { type: ButtonType.Normal }).height(46).layoutWeight(1).fontSize(13)
            .fontColor('#196B52').backgroundColor('#FFFFFF').border({ width: 1, color: '#DCE6DC' }).borderRadius(10)
            .onClick(() => this.leave())
          if (this.terminal()) {
            Button('重新连接', { type: ButtonType.Normal }).height(46).layoutWeight(1).fontSize(13)
              .fontColor('#FFFFFF').backgroundColor('#196B52').borderRadius(10).onClick(() => this.retry())
          } else {
            Button(this.screen.approved ? '断开连接' : '取消请求', { type: ButtonType.Normal })
              .height(46).layoutWeight(1).fontSize(13).fontColor('#A8443C').backgroundColor('#FFF5F2').borderRadius(10)
              .onClick(() => this.disconnect())
          }
        }.width('100%')
        Text(this.accessMode ? '此连接仅限查看' : this.terminal() ? '当前没有有效授权 · 重连需要重新批准' :
          '接入批准后，由控制端开关已支持的键鼠能力')
          .fontSize(11).fontColor('#778780').textAlign(TextAlign.Center).lineHeight(18)
      }.width('100%').padding({ top: 14 }).border({ width: { top: 1 }, color: '#E4EAE5' })
    }.width(Math.min(this.areaWidth - 24, 500)).height(Math.min(this.areaHeight - 24, this.terminal() ? 400 : 650))
      .padding({ left: this.compact() ? 22 : 30, right: this.compact() ? 22 : 30,
        top: this.areaHeight < 480 ? 16 : 24, bottom: this.areaHeight < 480 ? 16 : 24 })
      .backgroundColor('#F6F8F5').borderRadius(20).clip(true)
  }

  build() {
    Stack({ alignContent: Alignment.Center }) {
      if (this.surfaceActive) {
        XComponent({ type: XComponentType.SURFACE, controller: this.controller })
          .width(this.surfaceWidth()).height(this.surfaceHeight()).backgroundColor('#10181B')
      }
      if (!this.pictureVisible()) {
        Stack({ alignContent: Alignment.Center }) {
          this.stateCard()
        }.width('100%').height('100%').backgroundColor('#344B45')
      } else {
        Text(this.inputNotice.length > 0 && this.inputPanel.length === 0 && this.screen.code !== 'INPUT_DISABLING' ? this.inputNotice : this.inputStatus()).fontSize(11).fontColor('#426A4D')
          .padding({ left: 12, right: 12, top: 9, bottom: 9 }).borderRadius(8).backgroundColor('#EDF4E6')
          .height(32).constraintSize({ maxWidth: this.areaWidth - 24 }).maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
          .position({ x: 12, y: this.areaHeight - 44 })
        if (this.toolbarCollapsed) {
          this.collapsedToolbar()
        } else {
          this.sessionToolbar()
        }
        if (this.toolsOpen) {
          this.toolsPanel()
        }
        if (this.inputPanel.length > 0 && this.readyForInput()) { this.inputControls() }
      }
    }.width('100%').height('100%').backgroundColor('#344B45').clip(true)
      .onAreaChange((_oldArea: Area, area: Area) => {
        const width: number = parseFloat(String(area.width));
        const height: number = parseFloat(String(area.height));
        if (width !== this.areaWidth || (height !== this.areaHeight && this.inputPanel !== 'keyboard')) { this.stopInput(); }
        if (width > 0) { this.areaWidth = width; }
        if (height > 0) { this.areaHeight = height; }
      })
  }
}
