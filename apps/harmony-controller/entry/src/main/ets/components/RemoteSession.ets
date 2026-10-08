import { ScreenState, connectionPathLabel, screenIsLive } from '../model/DeskModels';
import { SystemInputCommand } from '../model/InputModels';
import { SessionInputController, InputSessionState } from '../input/SessionInputController';
import { HarmonyInputAdapter } from '../input/HarmonyInputAdapter';
import { RemoteInputMethod } from '../service/RemoteInputMethod';
import { CanvasAvoidArea } from '../service/CanvasAvoidArea';
import { SessionKeepScreenOn } from '../service/SessionKeepScreenOn';
import { createSessionKeepScreenOn } from '../service/HarmonyKeepScreenOn';
import { CanvasInsets, CanvasAvoidRegion, remainingCanvasInsets } from '../input/CanvasModels';
import { SessionOrbDrag, SessionToolsLayout, sessionToolsLayout, SESSION_ORB_SIZE,
  SESSION_ORB_DRAG_THRESHOLD } from '../model/SessionToolsModels';

@Component
export struct RemoteSession {
  @Prop @Watch('inputAvailabilityChanged') screen: ScreenState = new ScreenState();
  @Prop deviceName: string = '';
  @Prop endpoint: string = '';
  @Prop accessMode: string = '';
  @Prop initialControl: boolean = true;
  @Prop @Watch('keepScreenOnChanged') surfaceActive: boolean = false;
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
  @State private toolsOpen: boolean = false;
  @State private detailsOpen: boolean = false;
  @State private orbPosition: number = 0.5;
  @State private inputState: InputSessionState = new InputSessionState();
  @State private safeInsets: CanvasInsets = new CanvasInsets();
  @State private safeAreaNotice: string = '';
  private orbDrag: SessionOrbDrag = new SessionOrbDrag();
  private viewportX: number = 0;
  private viewportY: number = 0;
  private avoidRegions: CanvasAvoidRegion[] = [];
  private avoidArea?: CanvasAvoidArea;
  private keepScreenOn?: SessionKeepScreenOn;
  private inputController?: SessionInputController;
  private inputAdapter?: HarmonyInputAdapter;
  private keyboard: RemoteInputMethod = new RemoteInputMethod(
    (command: SystemInputCommand) => this.inputController?.send(command) ?? false,
    (open: boolean, error: string) => this.inputController?.keyboardChanged(open, error));

  aboutToAppear(): void {
    this.keepScreenOn = createSessionKeepScreenOn(this.getUIContext());
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

  aboutToDisappear(): void { this.keepScreenOn?.stop(); this.avoidArea?.stop(); this.inputController?.stop(); }

  private readyForInput(): boolean { return this.inputState.ready; }

  private inputAvailabilityChanged(): void {
    this.keepScreenOnChanged();
    this.updateCanvasLayout();
    this.inputController?.update(this.screen, this.sessionVisible);
  }

  private keepScreenOnChanged(): void {
    this.keepScreenOn?.setEnabled(this.sessionVisible && this.surfaceActive && screenIsLive(this.screen));
  }

  private updateCanvasLayout(): void {
    this.safeInsets = remainingCanvasInsets({ x: this.viewportX, y: this.viewportY,
      width: this.areaWidth, height: this.areaHeight }, this.avoidRegions);
    this.inputController?.setViewport(this.areaWidth, this.areaHeight, this.safeInsets);
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
    if (this.toolsOpen) { this.closeTools(); return; }
    this.inputController?.blur();
    this.detailsOpen = false;
    this.toolsOpen = true;
  }

  private closeTools(): void {
    this.toolsOpen = false;
    this.detailsOpen = false;
    this.inputController?.focus();
  }

  private toolsLayout(): SessionToolsLayout {
    return sessionToolsLayout(this.areaWidth, this.areaHeight, this.safeInsets, this.orbPosition, this.detailsOpen);
  }

  private orbTouch(event: TouchEvent): void {
    event.stopPropagation();
    if (event.type === TouchType.Cancel) { this.orbDrag.cancel(); return; }
    for (const point of event.changedTouches) {
      if (event.type === TouchType.Down) {
        this.inputController?.blur();
        this.orbDrag.begin(point.id, point.windowY, this.toolsLayout().orbY);
      } else if (event.type === TouchType.Move) {
        const position = this.orbDrag.move(point.id, point.windowY, this.toolsLayout());
        if (position !== undefined) {
          this.toolsOpen = false;
          this.detailsOpen = false;
          this.orbPosition = position;
        }
      } else if (event.type === TouchType.Up) {
        this.orbDrag.end(point.id);
      }
    }
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

  private canvasWidth(): number { return this.inputState.canvas.imageWidth > 0 ? this.inputState.canvas.imageWidth / this.inputState.canvas.zoom : this.surfaceWidth(); }
  private canvasHeight(): number { return this.canvasWidth() / this.ratio(); }

  private leave(): void {
    this.inputController?.stop();
    this.toolsOpen = false;
    this.onBack();
  }

  private disconnect(): void {
    this.inputController?.stop();
    this.toolsOpen = false;
    this.onDisconnect();
  }

  private retry(): void {
    this.inputController?.stop();
    this.toolsOpen = false;
    this.onRetry();
  }

  @Builder
  private resetCanvasButton() {
    if (this.inputState.canvas.zoom > 1) {
      Button(`${this.inputState.canvas.zoom.toFixed(1)}× 还原`, { type: ButtonType.Normal })
        .height(36).fontSize(11).fontColor('#196B52')
        .backgroundColor('#F9FCF7').border({ width: 1, color: '#DCE6DC' }).borderRadius(18).clip(true)
        .padding({ left: 12, right: 12 })
        .position({ x: this.safeInsets.left + 12, y: this.safeInsets.top + 12 })
        .accessibilityText('还原画面到适应窗口并居中')
        .hitTestBehavior(HitTestMode.Block)
        .onTouch((event: TouchEvent) => event.stopPropagation())
        .onClick(() => { this.inputController?.resetView(); this.inputController?.focus(); })
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
  private modeSwitch() {
    Row({ space: 3 }) {
      Button('仅查看', { type: ButtonType.Normal }).fontSize(12).height(38).fontColor('#196B52')
        .layoutWeight(1).backgroundColor(this.inputState.controlMode ? Color.Transparent : '#FFFFFF').borderRadius(6)
        .onClick(() => { this.toolsOpen = false; this.inputController?.stop(); })
      Button(this.screen.code === 'INPUT_ENABLING' ? '开启中' : '控制', { type: ButtonType.Normal }).fontSize(12).height(38)
        .layoutWeight(1).borderRadius(6).backgroundColor(this.inputState.controlMode ? '#FFFFFF' : Color.Transparent)
        .fontColor('#196B52').enabled(this.inputState.canControl)
        .onClick(() => { this.toolsOpen = false; this.inputController?.selectControl(true); })
    }.width('100%').padding(3).backgroundColor('#EEF3E9').border({ width: 1, color: '#DCE6D7' })
      .borderRadius(9).clip(true)
  }

  @Builder
  private floatingOrb() {
    Button({ type: ButtonType.Normal }) {
      Image($r('app.media.desk_session_monitor')).width(21).height(21)
    }.width(SESSION_ORB_SIZE).height(SESSION_ORB_SIZE)
      .backgroundColor('#F9FCF7').border({ width: 1, color: '#DCE6DC' })
      .borderRadius(SESSION_ORB_SIZE / 2).clip(true)
      .shadow({ radius: 10, color: '#33000000', offsetY: 3 })
      .position({ x: this.toolsLayout().orbX, y: this.toolsLayout().orbY })
      .accessibilityText(this.toolsOpen ? '收起会话工具' : '展开会话工具，可上下拖动')
      .hitTestBehavior(HitTestMode.Block)
      .onTouch((event: TouchEvent) => this.orbTouch(event))
      .onClick(() => this.toggleTools(), SESSION_ORB_DRAG_THRESHOLD)
  }

  @Builder
  private toolsActions() {
    Column({ space: 8 }) {
      this.modeSwitch()
      Row({ space: 8 }) {
        Button({ type: ButtonType.Normal }) {
          Row({ space: 5 }) {
            Image($r('app.media.desk_session_keyboard')).width(18).height(18)
            Text('键盘').fontSize(12).fontColor('#426A4D')
          }
        }.layoutWeight(1).height(44).borderRadius(8).backgroundColor('#EEF3E9')
          .enabled(this.readyForInput()).accessibilityText('打开系统键盘')
          .onClick(() => { this.toolsOpen = false; this.inputController?.toggleKeyboard(); })
        Button(this.inputState.canvas.touchMode === 'pointer' ? '指针模式' : '触屏模式', { type: ButtonType.Normal })
          .layoutWeight(1).height(44).fontSize(12).fontColor('#426A4D').borderRadius(8).backgroundColor('#EEF3E9')
          .accessibilityText(this.inputState.canvas.touchMode === 'pointer' ? '切换到触屏模式' : '切换到指针模式')
          .onClick(() => {
            this.inputController?.setTouchMode(this.inputState.canvas.touchMode === 'pointer' ? 'direct' : 'pointer');
            this.closeTools();
          })
      }.width('100%')
      Row({ space: 8 }) {
        Button('还原画面', { type: ButtonType.Normal }).layoutWeight(1).height(44).fontSize(12)
          .fontColor('#426A4D').borderRadius(8).backgroundColor('#EEF3E9')
          .onClick(() => { this.inputController?.resetView(); this.closeTools(); })
        Button('状态详情', { type: ButtonType.Normal }).layoutWeight(1).height(44).fontSize(12)
          .fontColor('#426A4D').borderRadius(8).backgroundColor('#EEF3E9')
          .onClick(() => { this.detailsOpen = true; })
      }.width('100%')
      Row({ space: 8 }) {
        Button('返回设备', { type: ButtonType.Normal }).layoutWeight(1).height(44).fontSize(12)
          .fontColor('#426A4D').borderRadius(8).backgroundColor('#EEF3E9')
          .accessibilityText('返回设备，保持连接').onClick(() => this.leave())
        Button('断开', { type: ButtonType.Normal }).layoutWeight(1).height(44).fontSize(12)
          .fontColor('#A8443C').borderRadius(8).backgroundColor('#FFF0EB')
          .onClick(() => this.disconnect())
      }.width('100%')
      Text(this.inputStatus()).fontSize(11).fontColor('#778780').lineHeight(17).width('100%')
    }.width('100%')
  }

  @Builder
  private toolsBackdrop() {
    Column() {
    }.width('100%').height('100%').backgroundColor(Color.Transparent)
      .hitTestBehavior(HitTestMode.Block)
      .onTouch((event: TouchEvent) => event.stopPropagation())
      .onMouse((event: MouseEvent) => event.stopPropagation())
      .onClick(() => this.closeTools())
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
        if (this.detailsOpen) {
          this.iconButton($r('app.media.desk_session_back'), '返回会话工具', () => { this.detailsOpen = false; })
        }
        Text(this.detailsOpen ? '状态详情' : this.deviceName || '远程设备')
          .fontSize(13).fontWeight(FontWeight.Medium).fontColor('#20352F').layoutWeight(1)
          .maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
        this.iconButton($r('app.media.desk_session_close'), '关闭会话工具', () => this.closeTools())
      }.padding({ left: this.detailsOpen ? 0 : 12, right: 3 }).width('100%').height(44)
      Scroll() {
        Column({ space: 8 }) {
          if (this.detailsOpen) {
            Column({ space: 7 }) {
              Text(this.inputStatus()).fontSize(13).fontWeight(FontWeight.Medium).fontColor('#196B52')
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
            this.stat('画质', () => 'VP8 软件视频 · 画质切换尚未实现')
            Text(this.accessMode ? '此连接仅可查看。Windows 可随时撤销已登记设备。' :
              '本次批准仅用于当前连接。断开或重新连接后，需要再次在 Windows 本机批准。')
              .fontSize(12).fontColor('#778780').lineHeight(20).width('100%')
          } else {
            this.toolsActions()
          }
        }.width('100%').padding({ left: 12, right: 12, top: 4, bottom: 12 })
      }.width('100%').layoutWeight(1).scrollBar(BarState.Auto)
    }.width(this.toolsLayout().panelWidth).height(this.toolsLayout().panelHeight)
      .backgroundColor('#FCFEFA').border({ width: 1, color: '#DCE6DC' }).borderRadius(15).clip(true)
      .position({ x: this.toolsLayout().panelX, y: this.toolsLayout().panelY })
      .hitTestBehavior(HitTestMode.Block)
      .onTouch((event: TouchEvent) => event.stopPropagation())
      .onMouse((event: MouseEvent) => event.stopPropagation())
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
          XComponent({ type: XComponentType.SURFACE, controller: this.controller })
            .width(this.canvasWidth()).height(this.canvasHeight()).backgroundColor('#10181B')
            .scale({ x: this.inputState.canvas.zoom, y: this.inputState.canvas.zoom, centerX: 0, centerY: 0 })
            .position({ x: this.inputState.canvas.imageX, y: this.inputState.canvas.imageY })
            .hitTestBehavior(HitTestMode.None)
        }.width('100%').height('100%').backgroundColor('#10181B').clip(true)
          .id('remote-desktop-input').focusable(true).focusOnTouch(true)
          .onTouch((event: TouchEvent) => { if (!this.toolsOpen) { this.inputAdapter?.touch(event); } })
          .onMouse((event: MouseEvent) => { if (!this.toolsOpen) { this.inputAdapter?.mouse(event); } })
          .onAxisEvent((event: AxisEvent) => { if (!this.toolsOpen) { this.inputAdapter?.wheel(event); } })
          .onKeyPreIme((event: KeyEvent) => (!this.toolsOpen && (this.inputAdapter?.key(event) ?? false)))
          .onKeyEvent((event: KeyEvent) => (!this.toolsOpen && (this.inputAdapter?.key(event) ?? false)))
          .onBlur(() => this.inputController?.blur())
      }
      if (!this.pictureVisible()) {
        Stack({ alignContent: Alignment.Center }) {
          this.stateCard()
        }.width('100%').height('100%').backgroundColor('#344B45')
      } else {
        if (this.safeAreaNotice || this.inputState.notice || this.inputState.canvas.dragReady ||
          this.inputState.canvas.gesture === 'dragging') {
          Text(this.safeAreaNotice || this.inputState.notice || this.inputStatus())
            .fontSize(12).fontColor('#EDF4E6').backgroundColor('#D910181B').borderRadius(8)
            .padding(8).maxLines(2).width(Math.min(this.areaWidth - this.safeInsets.left - this.safeInsets.right - 24, 320))
            .position({ x: this.safeInsets.left + 12, y: Math.max(0, this.areaHeight - this.safeInsets.bottom - 52) })
            .hitTestBehavior(HitTestMode.None)
        }
        this.resetCanvasButton()
        if (this.toolsOpen) {
          this.toolsBackdrop()
          this.toolsPanel()
        }
        this.floatingOrb()
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
