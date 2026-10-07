import { ScreenState } from '../model/DeskModels';
import { SessionInputMode, canRequestSystemInput, canSendSystemInput } from '../model/InputModels';
import type { SystemInputCommand } from '../model/InputModels';
import type { NativeInputEvent, TouchInputEvent, MouseInputEvent, WheelInputEvent, KeyInputEvent } from './InputEvents';

export class InputSessionState {
  controlMode: boolean = true;
  ready: boolean = false;
  canControl: boolean = false;
  keyboardOpen: boolean = false;
  notice: string = '';
  zoom: number = 1;
  imageX: number = 0;
  imageY: number = 0;
  imageWidth: number = 0;
  imageHeight: number = 0;
  safeX: number = 0;
  safeY: number = 0;
  safeWidth: number = 0;
  safeHeight: number = 0;
  dragReady: boolean = false;
  resetVisible: boolean = false;
  gesture: string = 'idle';
  canvas: { zoom: number, resetVisible: boolean, dragReady: boolean } = { zoom: 1, resetVisible: false, dragReady: false };
}

export interface InputSessionHost {
  send(command: SystemInputCommand): number;
  sendEvent(eventJson: string): number;
  reset(): void;
  setEnabled(enabled: boolean): number;
  release(): void;
  focus(): void;
  openKeyboard(): void;
  closeKeyboard(): void;
  canvasState(): string;
  changed(state: InputSessionState): void;
}

export class SessionInputController {
  state: InputSessionState = new InputSessionState();
  private host: InputSessionHost;
  private mode: SessionInputMode;
  private accessMode: string;
  private screen: ScreenState = new ScreenState();
  private visible: boolean = false;
  private width: number = 0;
  private height: number = 0;
  private remoteWidth: number = 0;
  private remoteHeight: number = 0;
  private keyboardOpen: boolean = false;
  private notice: string = '';
  private wasReady: boolean = false;
  private holdTimer: number = -1;

  constructor(host: InputSessionHost, initialControl: boolean = true, accessMode: string = '') {
    this.host = host;
    this.mode = new SessionInputMode(initialControl, accessMode);
    this.accessMode = accessMode;
    this.publish();
  }

  update(screen: ScreenState, visible: boolean): void {
    this.screen = screen;
    this.visible = visible;
    this.configureCanvas();
    if (!visible || screen.phase === 'ended' || screen.phase === 'failed' || screen.phase === 'idle') {
      this.stop();
      return;
    }
    const requested = this.mode.requested;
    this.mode.settle(screen);
    if (requested && !this.mode.controlMode) {
      this.stop(false);
      this.notice = '键鼠未启用或已关闭，当前仅查看。';
    } else if (this.mode.requestIfReady(screen, visible) && this.host.setEnabled(true) !== 0) {
      this.stop(false);
      this.notice = '键鼠开启失败，可稍后点击控制重试。';
    }
    const ready = this.ready();
    if (ready && !this.wasReady && !this.keyboardOpen) { this.focus(); }
    this.wasReady = ready;
    this.configureCanvas();
    this.publish();
  }

  setViewport(width: number, height: number, remoteWidth: number = this.remoteWidth,
    remoteHeight: number = this.remoteHeight): void {
    this.width = width;
    this.height = height;
    this.remoteWidth = remoteWidth;
    this.remoteHeight = remoteHeight;
    this.configureCanvas();
  }

  private configureCanvas(): void {
    const remoteWidth = this.remoteWidth > 0 ? this.remoteWidth : this.screen.width;
    const remoteHeight = this.remoteHeight > 0 ? this.remoteHeight : this.screen.height;
    if (this.width <= 0 || this.height <= 0 || remoteWidth <= 0 || remoteHeight <= 0) { return; }
    this.host.sendEvent(JSON.stringify({ kind: 'configure', viewportWidth: this.width,
      viewportHeight: this.height, remoteWidth: remoteWidth, remoteHeight: remoteHeight,
      insets: { left: 0, top: 0, right: 0, bottom: 0 }, padding: 24, enabled: this.ready() }));
    this.refreshCanvasState();
  }

  private refreshCanvasState(): void {
    try {
      const value = JSON.parse(this.host.canvasState());
      if (typeof value.zoom === 'number') {
        this.state.zoom = value.zoom; this.state.imageX = value.imageX ?? 0; this.state.imageY = value.imageY ?? 0;
        this.state.imageWidth = value.imageWidth ?? 0; this.state.imageHeight = value.imageHeight ?? 0;
        this.state.safeX = value.safeX ?? 0; this.state.safeY = value.safeY ?? 0;
        this.state.safeWidth = value.safeWidth ?? 0; this.state.safeHeight = value.safeHeight ?? 0;
        this.state.dragReady = value.dragReady === true; this.state.resetVisible = value.resetVisible === true;
        this.state.gesture = value.gesture ?? 'idle';
      }
    } catch { /* State remains usable while the native canvas is unavailable. */ }
  }

  resetView(): void {
    if (!this.ready()) { return; }
    this.dispatch({ kind: 'reset_view' });
    this.refreshCanvasState();
    this.publish();
  }

  canvasState(value?: Record<string, Object>): void {
    if (value) {
      this.state.zoom = Number(value.zoom ?? 1);
      this.state.resetVisible = value.resetVisible === true;
      this.state.dragReady = value.dragReady === true;
      this.publish();
      return;
    }
    this.refreshCanvasState();
    this.publish();
  }

  selectControl(enabled: boolean): void {
    if (!enabled) { this.stop(); return; }
    if (!canRequestSystemInput(this.screen, this.visible) || this.accessMode) { return; }
    if (!this.mode.controlMode) {
      this.mode.select(true);
      this.update(this.screen, this.visible);
    }
    this.notice = '';
    this.focus();
    this.publish();
  }

  toggleKeyboard(): void {
    if (this.keyboardOpen) { this.host.closeKeyboard(); return; }
    if (!this.ready()) { return; }
    this.release();
    this.focus();
    this.host.openKeyboard();
  }

  keyboardChanged(open: boolean, error: string): void {
    this.keyboardOpen = open;
    if (error) { this.notice = error; }
    this.publish();
  }

  blur(): void {
    this.release();
    this.host.closeKeyboard();
  }

  stop(notifyServer: boolean = true): void {
    const shouldDisable = this.mode.requested || this.screen.inputGranted || this.screen.code === 'INPUT_ENABLING';
    this.mode.select(false);
    this.wasReady = false;
    this.host.reset();
    this.host.closeKeyboard();
    this.notice = '';
    if (notifyServer && shouldDisable && this.host.setEnabled(false) !== 0) {
      this.notice = '无法确认键鼠关闭，正在结束连接。';
    }
    this.publish();
  }

  touch(event: TouchInputEvent): boolean {
    if (!this.ready()) { return false; }
    if (event.action === 'down' && event.points.length <= 1 && event.changedPoints.length > 0) { this.focus(); }
    if (event.action === 'down') {
      this.clearHoldTimer();
      this.holdTimer = setTimeout(() => {
        this.dispatch({ kind: 'tick', time: Date.now() });
        this.refreshCanvasState(); this.publish();
      }, 1000);
    } else if (event.action === 'up' || event.action === 'cancel') { this.clearHoldTimer(); }
    this.dispatch({ kind: 'touch', action: event.action, points: event.points, changedPoints: event.changedPoints,
      time: event.time, width: this.width, height: this.height });
    this.refreshCanvasState(); this.publish();
    return true;
  }

  private clearHoldTimer(): void {
    if (this.holdTimer >= 0) { clearTimeout(this.holdTimer); this.holdTimer = -1; }
  }

  mouse(event: MouseInputEvent): boolean {
    if (!this.ready()) { return false; }
    if (event.action === 'press') { this.focus(); }
    this.dispatch({ kind: 'mouse', action: event.action, button: event.button, x: event.x, y: event.y,
      width: this.width, height: this.height });
    return true;
  }

  wheel(event: WheelInputEvent): boolean {
    if (!this.ready()) { return false; }
    this.dispatch({ kind: 'wheel', action: event.action, x: event.x, y: event.y, dx: event.dx, dy: event.dy,
      discrete: event.discrete, width: this.width, height: this.height });
    return true;
  }

  key(event: KeyInputEvent): boolean {
    if (!this.ready() || this.keyboardOpen || event.code.length === 0) { return false; }
    this.dispatch({ kind: 'key', action: event.action, physicalCode: event.physicalCode, code: event.code });
    return true;
  }

  send(command: SystemInputCommand): boolean {
    if (!this.ready()) { return false; }
    if (command.kind === 'text') { return this.dispatch({ kind: 'text', text: command.text ?? '' }); }
    return this.acceptResult(this.host.send(command));
  }

  private dispatch(event: NativeInputEvent): boolean {
    if (!this.ready()) { return false; }
    return this.acceptResult(this.host.sendEvent(JSON.stringify(event)));
  }

  private acceptResult(result: number): boolean {
    if (result === 0) { return true; }
    this.stop();
    this.notice = result === 2 ? '键鼠已关闭或不可用，当前仅查看。' : '输入未能排队，已暂停控制。请检查连接后重试。';
    this.publish();
    return false;
  }

  release(): void { this.host.release(); }

  focus(): void {
    if (!this.ready()) { return; }
    try { this.host.focus(); }
    catch { this.notice = '点击远程画面后可使用键盘。'; this.publish(); }
  }

  private ready(): boolean {
    return !this.accessMode && canSendSystemInput(this.screen, this.visible, this.mode.controlMode);
  }

  private publish(): void {
    const state = new InputSessionState();
    state.controlMode = this.mode.controlMode;
    state.ready = this.ready();
    state.canControl = !this.accessMode && canRequestSystemInput(this.screen, this.visible);
    state.keyboardOpen = this.keyboardOpen;
    state.notice = this.notice;
    state.zoom = this.state.zoom; state.imageX = this.state.imageX; state.imageY = this.state.imageY;
    state.imageWidth = this.state.imageWidth; state.imageHeight = this.state.imageHeight;
    state.safeX = this.state.safeX; state.safeY = this.state.safeY; state.safeWidth = this.state.safeWidth; state.safeHeight = this.state.safeHeight;
    state.dragReady = this.state.dragReady; state.resetVisible = this.state.resetVisible; state.gesture = this.state.gesture;
    state.canvas = { zoom: state.zoom, resetVisible: state.resetVisible, dragReady: state.dragReady };
    this.state = state;
    this.host.changed(state);
  }
}
