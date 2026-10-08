import { inputMethod } from '@kit.IMEKit';
import type { SystemInputCommand } from '../model/InputModels';

export class RemoteInputMethod {
  private send: (command: SystemInputCommand) => boolean;
  private stateChanged: (open: boolean, error: string) => void;
  private controller: inputMethod.InputMethodController | undefined = undefined;
  private wanted: boolean = false;
  private tasks: Promise<void> = Promise.resolve();

  constructor(send: (command: SystemInputCommand) => boolean,
    stateChanged: (open: boolean, error: string) => void) {
    this.send = send;
    this.stateChanged = stateChanged;
  }

  open(): void {
    if (this.wanted) { return; }
    this.wanted = true;
    this.stateChanged(true, '');
    this.schedule();
  }

  close(): void {
    this.wanted = false;
    this.stateChanged(false, '');
    this.schedule();
  }

  private schedule(): void {
    this.tasks = this.tasks.then(async () => {
      try {
        if (this.wanted) { await this.attach(); }
        else { await this.detach(); }
      } catch {
        this.wanted = false;
        let message = '系统键盘操作失败，请重新打开键盘。';
        try { await this.detach(); }
        catch { message = '系统键盘关闭失败，请返回设备后重试。'; }
        this.stateChanged(false, message);
      }
    });
  }

  private async attach(): Promise<void> {
    if (this.controller) { return; }
    const controller = inputMethod.getController();
    // API 10 client APIs need no IME-management permission. Bind before showing so close can cancel a pending open.
    await controller.attach(false, {
      inputAttribute: {
        textInputType: inputMethod.TextInputType.TEXT,
        enterKeyType: inputMethod.EnterKeyType.NEWLINE
      }
    });
    this.controller = controller;
    if (!this.wanted) { await this.detach(); return; }
    // Do not advertise API 17 preview support: composition stays in the IME; insertText carries committed text.
    // https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/use-inputmethod-in-custom-edit-box
    controller.on('insertText', this.insertText);
    controller.on('deleteLeft', this.deleteLeft);
    controller.on('deleteRight', this.deleteRight);
    controller.on('sendFunctionKey', this.functionKey);
    controller.on('moveCursor', this.moveCursor);
    controller.on('sendKeyboardStatus', this.keyboardStatus);
    await controller.showTextInput();
    if (!this.wanted) { await this.detach(); }
  }

  private async detach(): Promise<void> {
    const controller = this.controller;
    this.controller = undefined;
    if (!controller) { return; }
    try {
      controller.off('insertText', this.insertText);
      controller.off('deleteLeft', this.deleteLeft);
      controller.off('deleteRight', this.deleteRight);
      controller.off('sendFunctionKey', this.functionKey);
      controller.off('moveCursor', this.moveCursor);
      controller.off('sendKeyboardStatus', this.keyboardStatus);
      await controller.hideTextInput();
    } finally {
      await controller.detach();
    }
  }

  private dispatch(command: SystemInputCommand): boolean {
    if (!this.wanted || !this.controller) { return false; }
    if (this.send(command)) { return true; }
    this.close();
    return false;
  }

  private tapKey(code: string): boolean {
    return this.dispatch({ kind: 'key', code: code, down: true }) &&
      this.dispatch({ kind: 'key', code: code, down: false });
  }

  private deleteText(code: string, length: number): void {
    if (!Number.isFinite(length) || length <= 0) { return; }
    for (let index = 0; index < Math.floor(length); index++) {
      if (!this.tapKey(code)) { return; }
    }
  }

  private insertText = (text: string): void => {
    if (text.length > 0) { this.dispatch({ kind: 'text', text: text }); }
  };
  private deleteLeft = (length: number): void => { this.deleteText('Backspace', length); };
  private deleteRight = (length: number): void => { this.deleteText('Delete', length); };
  private functionKey = (key: inputMethod.FunctionKey): void => {
    switch (key.enterKeyType) {
      case inputMethod.EnterKeyType.NONE:
        return;
      case inputMethod.EnterKeyType.NEXT:
        this.tapKey('Tab');
        return;
      case inputMethod.EnterKeyType.PREVIOUS:
        if (this.dispatch({ kind: 'key', code: 'Shift', down: true })) {
          this.tapKey('Tab');
          this.dispatch({ kind: 'key', code: 'Shift', down: false });
        }
        return;
      case inputMethod.EnterKeyType.UNSPECIFIED:
      case inputMethod.EnterKeyType.GO:
      case inputMethod.EnterKeyType.SEARCH:
      case inputMethod.EnterKeyType.SEND:
      case inputMethod.EnterKeyType.DONE:
      case inputMethod.EnterKeyType.NEWLINE:
        this.tapKey('Enter');
        return;
      default:
        return;
    }
  };
  private moveCursor = (direction: inputMethod.Direction): void => {
    switch (direction) {
      case inputMethod.Direction.CURSOR_UP: this.tapKey('ArrowUp'); break;
      case inputMethod.Direction.CURSOR_DOWN: this.tapKey('ArrowDown'); break;
      case inputMethod.Direction.CURSOR_LEFT: this.tapKey('ArrowLeft'); break;
      case inputMethod.Direction.CURSOR_RIGHT: this.tapKey('ArrowRight'); break;
    }
  };
  private keyboardStatus = (status: inputMethod.KeyboardStatus): void => {
    if (status === inputMethod.KeyboardStatus.HIDE && this.wanted) { this.close(); }
  };
}
