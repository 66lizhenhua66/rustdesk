import { ScreenState } from '../model/DeskModels';
import { SessionInputMode, canRequestSystemInput, canSendSystemInput } from '../model/InputModels';
import { CanvasInsets, CanvasState, decodeCanvasState } from './CanvasModels';
import type { SystemInputCommand } from '../model/InputModels';
import type { NativeInputEvent, TouchInputEvent, MouseInputEvent, WheelInputEvent, KeyInputEvent } from './InputEvents';

export class InputSessionState {
  controlMode: boolean = true;
  ready: boolean = false;
  canControl: boolean = false;
  keyboardOpen: boolean = false;
  notice: string = '';
  canvas: CanvasState = new CanvasState();
}

interface CanvasConfiguration {
  kind: string;
  viewportWidth: number;
  viewportHeight: number;
  remoteWidth: number;
  remoteHeight: number;
  insets: CanvasInsets;
  padding: number;
  enabled: boolean;
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
  private insets: CanvasInsets = new CanvasInsets();
  private canvas: CanvasState = new CanvasState();
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

  setViewport(width: number, height: number, insets: CanvasInsets = new CanvasInsets()): void {
    this.width = Math.max(0, width);
    this.height = Math.max(0, height);
    this.insets = insets;
    this.configureCanvas();
    this.publish();
  }

  private canView(): boolean {
    return this.visible && this.screen.approved && this.screen.rendered > 0 &&
      this.screen.phase === 'viewing';
  }

  private configureCanvas(): void {
    if (!this.visible || !this.screen.approved ||
      (this.screen.phase !== 'approved' && this.screen.phase !== 'viewing') ||
      this.screen.width <= 0 || this.screen.height <= 0) { return; }
    const event: CanvasConfiguration = { kind: 'configure', viewportWidth: this.width,
      viewportHeight: this.height, remoteWidth: this.screen.width, remoteHeight: this.screen.height,
      insets: this.insets, padding: 8, enabled: this.ready() };
    this.nativeEvent(JSON.stringify(event));
  }

  private refreshCanvas(): boolean {
    try {
      this.canvas = decodeCanvasState(this.host.canvasState());
      this.scheduleTick();
      return true;
    } catch {
      this.fail(1);
      this.notice = '画布状态读取失败，已停止输入，请重新连接。';
      return false;
    }
  }

  resetView(): void { this.viewEvent({ kind: 'reset_view' }); }

  setTouchMode(mode: string): void {
    if (mode === 'direct' || mode === 'pointer') { this.viewEvent({ kind: 'touch_mode', mode: mode }); }
  }

  selectControl(enabled: boolean): void {
    if (!enabled) { this.stop(); return; }
    if (!canRequestSystemInput(this.screen, this.visible) || this.accessMode) { return; }
    this.notice = '';
    if (!this.mode.controlMode) {
      this.mode.select(true);
      this.update(this.screen, this.visible);
    }
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
    this.clearHoldTimer();
    this.host.reset();
    this.host.closeKeyboard();
    this.notice = '';
    if (notifyServer && shouldDisable && this.host.setEnabled(false) !== 0) {
      this.notice = '无法确认键鼠关闭，正在结束连接。';
    }
    this.canvas.dragReady = false;
    this.canvas.inputEnabled = false;
    this.canvas.nextTickMs = 0;
    this.canvas.gesture = 'idle';
    this.publish();
  }

  touch(event: TouchInputEvent): boolean {
    if (!this.canView()) { return false; }
    if (event.action === 'down' && event.points.length <= 1 && event.changedPoints.length > 0) { this.focus(); }
    this.viewEvent({ kind: 'touch', action: event.action, points: event.points,
      changedPoints: event.changedPoints, time: event.time });
    return true;
  }

  private clearHoldTimer(): void {
    if (this.holdTimer !== -1) { clearTimeout(this.holdTimer); this.holdTimer = -1; }
  }

  private scheduleTick(): void {
    this.clearHoldTimer();
    if (!this.canView() || this.canvas.nextTickMs <= 0) { return; }
    this.holdTimer = setTimeout(() => {
      this.holdTimer = -1;
      this.viewEvent({ kind: 'tick', time: Date.now() });
    }, Math.max(0, this.canvas.nextTickMs - Date.now()));
  }

  mouse(event: MouseInputEvent): boolean {
    if (!this.ready()) { return false; }
    if (event.action === 'press') { this.focus(); }
    this.viewEvent({ kind: 'mouse', action: event.action, button: event.button, x: event.x, y: event.y });
    return true;
  }

  wheel(event: WheelInputEvent): boolean {
    if (!this.ready()) { return false; }
    this.viewEvent({ kind: 'wheel', action: event.action, x: event.x, y: event.y,
      dx: event.dx, dy: event.dy, discrete: event.discrete });
    return true;
  }

  key(event: KeyInputEvent): boolean {
    if (!this.ready() || this.keyboardOpen || event.code.length === 0) { return false; }
    this.viewEvent({ kind: 'key', action: event.action, physicalCode: event.physicalCode, code: event.code });
    return true;
  }

  send(command: SystemInputCommand): boolean {
    if (!this.ready()) { return false; }
    if (command.kind === 'text') { return this.viewEvent({ kind: 'text', text: command.text ?? '' }); }
    const result = this.host.send(command);
    if (result !== 0) { this.fail(result); this.publish(); }
    return result === 0;
  }

  private viewEvent(event: NativeInputEvent): boolean {
    if (!this.canView()) { return false; }
    const accepted = this.nativeEvent(JSON.stringify(event));
    this.publish();
    return accepted;
  }

  private nativeEvent(raw: string): boolean {
    const result = this.host.sendEvent(raw);
    if (result !== 0) { this.fail(result); return false; }
    return this.refreshCanvas();
  }

  private fail(result: number): void {
    this.stop();
    this.notice = result === 2 ? '键鼠已关闭或不可用，当前仅查看。' :
      '输入或画布操作失败，已暂停控制，请检查连接后重试。';
  }

  release(): void {
    this.clearHoldTimer();
    this.host.release();
    if (this.canView()) { this.refreshCanvas(); }
    this.publish();
  }

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
    state.canvas = this.canvas;
    this.state = state;
    this.host.changed(state);
  }
}
