import { ScreenState } from '../model/DeskModels';

@Component
export struct RemoteSession {
  @Prop screen: ScreenState = new ScreenState();
  @Prop deviceName: string = '';
  @Prop endpoint: string = '';
  @Prop surfaceActive: boolean = false;
  controller: XComponentController = new XComponentController();
  onBack: () => void = () => {};
  onDisconnect: () => void = () => {};
  onRetry: () => void = () => {};
  @State private areaWidth: number = 390;
  @State private areaHeight: number = 720;
  @State private toolbarCollapsed: boolean = false;
  @State private toolsOpen: boolean = false;

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

  private leave(): void {
    this.toolsOpen = false;
    this.onBack();
  }

  private disconnect(): void {
    this.toolsOpen = false;
    this.toolbarCollapsed = false;
    this.onDisconnect();
  }

  private retry(): void {
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
      Text('仅查看').fontSize(12).fontWeight(FontWeight.Medium).fontColor('#196B52')
        .padding({ left: 9, right: 9, top: 10, bottom: 10 }).backgroundColor('#FFFFFF').borderRadius(6)
      Button('控制·未实现', { type: ButtonType.Normal }).fontSize(10).height(38)
        .padding({ left: 6, right: 6 }).backgroundColor(Color.Transparent).fontColor('#778780').enabled(false)
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
      this.unavailableTool($r('app.media.desk_session_keyboard'), '键盘')
      Button({ type: ButtonType.Normal }) {
        Column({ space: 2 }) {
          Image($r('app.media.desk_session_settings')).width(18).height(18)
          Text('工具').fontSize(10).fontColor('#426A4D')
        }
      }.width(44).height(44).borderRadius(8)
        .backgroundColor(this.toolsOpen ? '#E4EFDF' : Color.Transparent)
        .accessibilityText(this.toolsOpen ? '关闭会话工具' : '打开会话工具')
        .onClick(() => { this.toolsOpen = !this.toolsOpen; })
    }
  }

  @Builder
  private endButtons() {
    Row({ space: 1 }) {
      Button('断开', { type: ButtonType.Normal }).height(44).fontSize(12).fontColor('#A8443C')
        .padding({ left: 12, right: 12 }).borderRadius(8).backgroundColor('#FFF5F2')
        .onClick(() => this.disconnect())
      this.iconButton($r('app.media.desk_session_up'), '收起工具栏', () => {
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
        Text('仅查看').fontSize(10).fontColor('#426A4D').padding(5).backgroundColor('#E3EFDB').borderRadius(5)
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
        this.iconButton($r('app.media.desk_session_close'), '关闭会话工具', () => { this.toolsOpen = false; })
      }.padding({ left: 18, right: 5 }).width('100%')
      Scroll() {
        Column({ space: 12 }) {
          Column({ space: 7 }) {
            Text('当前仅允许查看').fontSize(15).fontWeight(FontWeight.Medium).fontColor('#196B52')
            Text('触摸画面不会操作电脑。键鼠控制、键盘输入和文件传输尚未实现。')
              .fontSize(12).fontColor('#778780').lineHeight(20)
          }.alignItems(HorizontalAlign.Start).width('100%').padding(14).borderRadius(10).backgroundColor('#EDF4E6')
          this.stat('连接地址', () => this.endpoint || '未提供')
          this.stat('设备身份', () => this.screen.verified ? '已通过可信身份核验' : '尚未完成核验')
          this.stat('本次授权', () => this.screen.approved ? 'Windows 本机已批准查看' : '尚未批准查看')
          this.stat('远端画面', () => this.screen.approved && this.screen.width > 0 && this.screen.height > 0 ?
            `${this.screen.width} × ${this.screen.height}` : '等待画面协商')
          this.stat('实际接收 / 呈现', () => `${this.screen.frames} 帧 / ${this.screen.rendered} 帧`)
          this.stat('已接收视频数据', () => `${Math.round(this.screen.bytes / 1024)} KiB`)
          this.stat('画面显示', () => '适应窗口 · 保持远端比例')
          this.stat('画质', () => 'VP8 软件视频 · 画质切换尚未实现')
          Text('本次批准仅用于当前连接。断开或重新连接后，需要再次在 Windows 本机批准。')
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
            Text(this.terminal() ? 'REMOTE DESK' : 'WAITING FOR CONFIRMATION')
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
              this.progressStep('等待本机批准查看', () => this.screen.approved ? '已批准，正在等待首帧呈现' : '请在 Windows 连接管理器中接受请求',
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
        Text(this.terminal() ? '当前没有有效授权 · 重连需要重新批准' : '仅请求查看 · 键盘、鼠标和文件权限未开放')
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
        Text('仅查看 · 键鼠尚未开放').fontSize(11).fontColor('#426A4D')
          .padding({ left: 12, right: 12, top: 9, bottom: 9 }).borderRadius(8).backgroundColor('#EDF4E6')
          .height(32).position({ x: 12, y: this.areaHeight - 44 })
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
      })
  }
}
