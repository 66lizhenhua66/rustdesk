import { ScreenState, connectionPathLabel } from '../model/DeskModels';
import { SystemInputCommand } from '../model/InputModels';
import { SessionInputController, InputSessionState } from '../input/SessionInputController';
import { HarmonyInputAdapter } from '../input/HarmonyInputAdapter';
import { RemoteInputMethod } from '../service/RemoteInputMethod';
import { CanvasAvoidArea } from '../service/CanvasAvoidArea';
import { CanvasInsets, CanvasAvoidRegion, remainingCanvasInsets } from '../input/CanvasModels';

@Component
export struct RemoteSession {
  @Prop @Watch('inputAvailabilityChanged') screen: ScreenState = new ScreenState();
  @Prop deviceName: string = '';
  @Prop endpoint: string = '';
  @Prop accessMode: string = '';
  @Prop initialControl: boolean = true;
  @Prop surfaceActive: boolean = false;
  @Prop @Watch('inputAvailabilityChanged') sessionVisible: boolean = true;
  controller: XComponentController = new XComponentController();
  onBack: () => void = () => {};
  onDisconnect: () => void = () => {};
  onRetry: () => void = () => {};
  onInput: (command: string) => number = () => 1;
  onInputEvent: (event: string) => number = () => 1;
  onResetInput: () => void = () => {};
  onCanvasState: () => string = () => '{}';
  onReleaseInput: () => void = () => {};
  onSetInputEnabled: (enabled: boolean) => number = () => 1;
  @State private areaWidth: number = 390;
  @State private areaHeight: number = 720;
  @State private toolbarCollapsed: boolean = false;
  @State private toolsOpen: boolean = false;
  @State private inputState: InputSessionState = new InputSessionState();
  @State private safeInsets: CanvasInsets = new CanvasInsets();
  @State private safeAreaNotice: string = '';
  @State private toolbarHeight: number = 0;
  private viewportX: number = 0;
  private viewportY: number = 0;
  private avoidRegions: CanvasAvoidRegion[] = [];
  private avoidArea?: CanvasAvoidArea;
  private inputController?: SessionInputController;
  private inputAdapter?: HarmonyInputAdapter;
  private keyboard: RemoteInputMethod = new RemoteInputMethod(
    (command: SystemInputCommand) => this.inputController?.send(command) ?? false,
    (open: boolean, error: string) => this.inputController?.keyboardChanged(open, error));

  aboutToAppear(): void {
    this.inputController = new SessionInputController({
      send: (command: SystemInputCommand) => this.onInput(JSON.stringify(command)),
      sendEvent: (event: string) => this.onInputEvent(event),
      reset: () => this.onResetInput(),
      setEnabled: (enabled: boolean) => this.onSetInputEnabled(enabled),
      release: () => this.onReleaseInput(),
      focus: () => { this.getUIContext().getFocusController().requestFocus('remote-desktop-input'); },
      openKeyboard: () => this.keyboard.open(),
      closeKeyboard: () => this.keyboard.close(),
      canvasState: () => this.onCanvasState(),
      changed: (state: InputSessionState) => { this.inputState = state; }
    }, this.initialControl, this.accessMode);
    this.inputAdapter = new HarmonyInputAdapter(this.inputController);
    this.avoidArea = new CanvasAvoidArea(this.getUIContext(), (regions: CanvasAvoidRegion[], error: string) => {
      if (!error) { this.avoidRegions = regions; }
      this.safeAreaNotice = error;
      this.updateCanvasLayout();
    });
    this.avoidArea.start();
    this.inputAvailabilityChanged();
  }

  aboutToDisappear(): void { this.avoidArea?.stop(); this.inputController?.stop(); }

  private readyForInput(): boolean { return this.inputState.ready; }

  private inputAvailabilityChanged(): void {
    this.updateCanvasLayout();
    this.inputController?.update(this.screen, this.sessionVisible);
  }

  private updateCanvasLayout(): void {
    this.safeInsets = remainingCanvasInsets({ x: this.viewportX, y: this.viewportY,
      width: this.areaWidth, height: this.areaHeight }, this.avoidRegions);
    const insets = new CanvasInsets();
    insets.left = this.safeInsets.left;
    insets.right = this.safeInsets.right;
    insets.top = this.safeInsets.top + (this.toolbarCollapsed ? 48 : (this.toolbarHeight || (this.compact() ? 111 : 60)) + 12);
    insets.bottom = this.safeInsets.bottom + 56;
    this.inputController?.setViewport(this.areaWidth, this.areaHeight, insets);
  }

  private inputStatus(): string {
    if (this.accessMode) { return '只读查看 · 键鼠不可用'; }
    if (this.screen.code === 'INPUT_ENABLING') { return '正在启用键鼠 · 等待设备确认'; }
    if (this.screen.code === 'INPUT_DISABLING') { return '正在关闭键鼠 · 本地输入已停止'; }
    if (this.inputState.canvas.dragReady) { return '已进入拖动状态 · 移动手指拖动远程对象'; }
    if (this.inputState.canvas.gesture === 'dragging') { return '正在拖动远程对象 · 抬手释放'; }
    if (this.readyForInput()) { return this.inputState.canvas.touchMode === 'pointer' ?
      '指针模式 · 滑动移动鼠标，轻点左键，双指右键/滚动' : '触屏模式 · 滑动画布，长按 1 秒后拖动，双指右键/滚动'; }
    return this.screen.inputSupported ? '仅查看 · 点“控制”开始操作' : '仅查看 · 当前设备未开放键鼠能力';
  }

  private toggleTools(): void {
    this.inputController?.blur();
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

  private surfaceWidth(): number { return Math.max(1, Math.min(this.areaWidth, this.areaHeight * this.ratio())); }
  private surfaceHeight(): number { return this.surfaceWidth() / this.ratio(); }
  private panelTop(): number { return this.compact() ? 134 : 80; }

  private canvasWidth(): number { return this.inputState.canvas.imageWidth > 0 ? this.inputState.canvas.imageWidth / this.inputState.canvas.zoom : this.surfaceWidth(); }
  private canvasHeight(): number { return this.canvasWidth() / this.ratio(); }
  private canvasX(): number { return this.inputState.canvas.imageX - this.inputState.canvas.safeX; }
  private canvasY(): number { return this.inputState.canvas.imageY - this.inputState.canvas.safeY; }
  private safeWidth(): number { return this.screen.approved ? this.inputState.canvas.safeWidth : this.areaWidth; }
  private safeHeight(): number { return this.screen.approved ? this.inputState.canvas.safeHeight : this.areaHeight; }

  private leave(): void {
    this.inputController?.stop();
    this.toolsOpen = false;
    this.onBack();
  }

  private disconnect(): void {
    this.inputController?.stop();
    this.toolsOpen = false;
    this.toolbarCollapsed = false;
    this.onDisconnect();
  }

  private retry(): void {
    this.inputController?.stop();
    this.toolsOpen = false;
    this.toolbarCollapsed = false;
    this.onRetry();
  }

  @Builder
  private resetCanvasButton() {
    if (this.inputState.canvas.resetVisible) {
      Button('还原画面', { type: ButtonType.Normal }).height(38).fontSize(11).fontColor('#196B52')
        .backgroundColor('#F9FCF7').borderRadius(9).border({ width: 1, color: '#DCE6DC' })
        .width(94).focusOnTouch(false)
        .accessibilityText('还原画面到适应窗口并居中')
        .onClick(() => this.inputController?.resetView())
    }
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
        .padding({ left: 9, right: 9 }).backgroundColor(this.inputState.controlMode ? Color.Transparent : '#FFFFFF').borderRadius(6)
        .onClick(() => this.inputController?.stop())
      Button(this.screen.code === 'INPUT_ENABLING' ? '开启中' : '控制', { type: ButtonType.Normal }).fontSize(12).height(38)
        .padding({ left: 9, right: 9 }).borderRadius(6).backgroundColor(this.inputState.controlMode ? '#FFFFFF' : Color.Transparent)
        .fontColor('#196B52').enabled(this.inputState.canControl)
        .onClick(() => { this.toolsOpen = false; this.inputController?.selectControl(true); })
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
      }.width(44).height(44).borderRadius(8).backgroundColor(this.inputState.keyboardOpen ? '#E4EFDF' : Color.Transparent)
        .enabled(this.readyForInput()).focusOnTouch(false).accessibilityText(this.inputState.keyboardOpen ? '收起系统键盘' : '打开系统键盘')
        .onClick(() => this.inputController?.toggleKeyboard())
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
        this.inputController?.blur();
        this.toolsOpen = false;
        this.toolbarCollapsed = true;
        this.updateCanvasLayout();
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
    }.padding(8).width(Math.min(this.areaWidth - this.safeInsets.left - this.safeInsets.right - 24, 920)).borderRadius(14)
      .backgroundColor('#F9FCF7').border({ width: 1, color: '#DCE6DC' })
      .position({ x: this.safeInsets.left + (this.areaWidth - this.safeInsets.left - this.safeInsets.right - Math.min(this.areaWidth - this.safeInsets.left - this.safeInsets.right - 24, 920)) / 2, y: this.safeInsets.top + 12 })
      .onAreaChange((_old: Area, area: Area) => {
        const height = Number(area.height);
        if (height > 0 && height !== this.toolbarHeight) { this.toolbarHeight = height; this.updateCanvasLayout(); }
      })
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
      .position({ x: (this.areaWidth - Math.min(this.areaWidth - 24, 360)) / 2, y: this.safeInsets.top })
      .accessibilityText('展开会话工具栏，包含断开按钮')
      .onClick(() => { this.toolbarCollapsed = false; this.updateCanvasLayout(); })
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
        this.iconButton($r('app.media.desk_session_close'), '关闭会话工具', () => { this.toolsOpen = false; this.inputController?.focus(); })
      }.padding({ left: 18, right: 5 }).width('100%')
      Scroll() {
        Column({ space: 12 }) {
          Column({ space: 7 }) {
            Text(this.inputStatus()).fontSize(15).fontWeight(FontWeight.Medium).fontColor('#196B52')
            Text('轻点左键，双指轻点右键，双指滑动滚轮；捏合缩放 1～10 倍。单指滑动画布，长按 1 秒出现提示后拖动远程对象。')
              .fontSize(12).fontColor('#778780').lineHeight(20)
          }.alignItems(HorizontalAlign.Start).width('100%').padding(14).borderRadius(10).backgroundColor('#EDF4E6')
          this.stat('连接地址', () => this.endpoint || '未提供')
          this.stat('实际连接方式', () => connectionPathLabel(this.screen.connectionPath))
          this.stat('设备身份', () => this.screen.verified ? '已通过可信身份核验' : '尚未完成核验')
          this.stat('本次授权', () => !this.screen.approved ? '尚未确认' :
            this.screen.accessMode === 'unattended' ? '无人值守凭据已验证 · 只读' :
              this.screen.accessMode === 'pair' ? 'Windows 已批准登记 · 只读' : 'Windows 本机已批准连接')
          this.stat('键盘与鼠标', () => this.screen.code === 'INPUT_ENABLING' ? '正在开启，等待状态回执' : this.screen.code === 'INPUT_DISABLING' ? '正在关闭，等待状态回执' : this.screen.inputGranted ? '设备已确认开启' : '设备当前未开启')
          this.stat('远端画面', () => this.screen.approved && this.screen.width > 0 && this.screen.height > 0 ?
            `${this.screen.width} × ${this.screen.height}` : '等待画面协商')
          this.stat('实际接收 / 呈现', () => `${this.screen.frames} 帧 / ${this.screen.rendered} 帧`)
          this.stat('已接收视频数据', () => `${Math.round(this.screen.bytes / 1024)} KiB`)
          this.stat('画面显示', () => `${this.inputState.canvas.zoom.toFixed(1)} 倍 · 保持远端比例`)
          Row({ space: 8 }) {
            Button('触屏模式').fontSize(12).height(40).onClick(() => this.inputController?.setTouchMode('direct'))
            Button('指针模式').fontSize(12).height(40).onClick(() => this.inputController?.setTouchMode('pointer'))
          }
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
          (this.inputState.controlMode ? '批准后直接控制远程桌面' : '本次以仅查看模式进入桌面'))
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
        Stack({ alignContent: Alignment.TopStart }) {
          Stack({ alignContent: Alignment.TopStart }) {
            XComponent({ type: XComponentType.SURFACE, controller: this.controller })
              .width(this.canvasWidth()).height(this.canvasHeight()).backgroundColor('#10181B')
              .scale({ x: this.inputState.canvas.zoom, y: this.inputState.canvas.zoom, centerX: 0, centerY: 0 })
              .position({ x: this.canvasX(), y: this.canvasY() })
              .hitTestBehavior(HitTestMode.None)
          }.width(this.safeWidth()).height(this.safeHeight()).clip(true)
            .position({ x: this.inputState.canvas.safeX, y: this.inputState.canvas.safeY })
            .hitTestBehavior(HitTestMode.None)
        }.width('100%').height('100%').backgroundColor('#10181B').clip(true)
          .id('remote-desktop-input').focusable(true).focusOnTouch(true)
          .onTouch((event: TouchEvent) => this.inputAdapter?.touch(event))
          .onMouse((event: MouseEvent) => this.inputAdapter?.mouse(event))
          .onAxisEvent((event: AxisEvent) => this.inputAdapter?.wheel(event))
          .onKeyPreIme((event: KeyEvent) => (this.inputAdapter?.key(event) ?? false))
          .onKeyEvent((event: KeyEvent) => (this.inputAdapter?.key(event) ?? false))
          .onBlur(() => this.inputController?.blur())
      }
      if (!this.pictureVisible()) {
        Stack({ alignContent: Alignment.Center }) {
          this.stateCard()
        }.width('100%').height('100%').backgroundColor('#344B45')
      } else {
        Row({ space: 8 }) {
          Text(this.safeAreaNotice || (this.inputState.notice.length > 0 && this.screen.code !== 'INPUT_DISABLING' ? this.inputState.notice :
            `${this.inputState.canvas.zoom.toFixed(1)}× · ${this.inputStatus()}`)).fontSize(11).fontColor('#426A4D')
            .padding({ left: 12, right: 12, top: 8, bottom: 8 }).borderRadius(8).backgroundColor('#EDF4E6')
            .height(38).layoutWeight(1).maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
          this.resetCanvasButton()
        }.width(Math.max(1, this.areaWidth - this.safeInsets.left - this.safeInsets.right - 24))
          .position({ x: this.safeInsets.left + 12, y: Math.max(0, this.areaHeight - this.safeInsets.bottom - 50) })
        if (this.toolbarCollapsed) {
          this.collapsedToolbar()
        } else {
          this.sessionToolbar()
        }
        if (this.toolsOpen) {
          this.toolsPanel()
        }
      }
    }.width('100%').height('100%').backgroundColor('#344B45').clip(true)
      .onAreaChange((_oldArea: Area, area: Area) => {
        const width: number = parseFloat(String(area.width));
        const height: number = parseFloat(String(area.height));
        if (width > 0) { this.areaWidth = width; }
        if (height > 0) { this.areaHeight = height; }
        this.viewportX = Number(area.globalPosition.x);
        this.viewportY = Number(area.globalPosition.y);
        this.avoidArea?.refresh();
        this.updateCanvasLayout();
      })
  }
}
