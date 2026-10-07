import { KeyCode } from '@kit.InputKit';
import { InputPoint } from './InputEvents';
import { SessionInputController } from './SessionInputController';

export class HarmonyInputAdapter {
  private readonly input: SessionInputController;

  constructor(input: SessionInputController) {
    this.input = input;
  }

  touch(event: TouchEvent): void {
    // Mouse clicks also generate touch events; only the mouse handler forwards them.
    if (event.source === SourceType.Mouse) { return; }
    const points: InputPoint[] = [];
    const changedPoints: InputPoint[] = [];
    for (const point of event.touches) {
      points.push({ id: point.id, x: point.x, y: point.y });
    }
    for (const point of event.changedTouches) {
      changedPoints.push({ id: point.id, x: point.x, y: point.y });
    }
    const action = event.type === TouchType.Down ? 'down' : event.type === TouchType.Up ? 'up' :
      event.type === TouchType.Move ? 'move' : event.type === TouchType.Cancel ? 'cancel' : '';
    if (this.input.touch({ action: action, points: points, changedPoints: changedPoints, time: Date.now() })) {
      event.stopPropagation();
    }
  }

  mouse(event: MouseEvent): void {
    const action = event.action === MouseAction.Press ? 'press' : event.action === MouseAction.Release ? 'release' :
      event.action === MouseAction.CANCEL ? 'cancel' : 'move';
    const button = event.button === MouseButton.Left ? 'left' : event.button === MouseButton.Right ? 'right' :
      event.button === MouseButton.Middle ? 'middle' : '';
    if (this.input.mouse({ action: action, button: button, x: event.x, y: event.y })) {
      event.stopPropagation();
    }
  }

  wheel(event: AxisEvent): void {
    const action = event.action === AxisAction.BEGIN ? 'begin' : event.action === AxisAction.UPDATE ? 'update' : '';
    let dx = 0;
    let dy = 0;
    let discrete = false;
    if (action === 'begin' || action === 'update') {
      if (event.source === SourceType.Mouse && (event.axisVertical || event.axisHorizontal)) {
        dx = Math.sign(event.axisHorizontal ?? 0);
        dy = -Math.sign(event.axisVertical ?? 0);
        discrete = true;
      } else {
        dx = event.getHorizontalAxisValue();
        dy = -event.getVerticalAxisValue();
      }
    }
    this.input.wheel({ action: action, x: event.x, y: event.y, dx: dx, dy: dy, discrete: discrete });
  }

  key(event: KeyEvent): boolean {
    const action = event.type === KeyType.Down ? 'down' : event.type === KeyType.Up ? 'up' : '';
    return this.input.key({ action: action, physicalCode: event.keyCode, code: this.hardwareCode(event.keyCode) });
  }

  private hardwareCode(code: number): string {
    if (code >= KeyCode.KEYCODE_A && code <= KeyCode.KEYCODE_Z) { return 'Key' + String.fromCharCode(65 + code - KeyCode.KEYCODE_A); }
    if (code >= KeyCode.KEYCODE_0 && code <= KeyCode.KEYCODE_9) { return 'Digit' + String(code - KeyCode.KEYCODE_0); }
    if (code >= KeyCode.KEYCODE_F1 && code <= KeyCode.KEYCODE_F12) { return 'F' + String(1 + code - KeyCode.KEYCODE_F1); }
    if (code >= KeyCode.KEYCODE_NUMPAD_0 && code <= KeyCode.KEYCODE_NUMPAD_9) { return 'Numpad' + String(code - KeyCode.KEYCODE_NUMPAD_0); }
    switch (code) {
      case KeyCode.KEYCODE_MINUS: return 'Minus';
      case KeyCode.KEYCODE_EQUALS: return 'Equal';
      case KeyCode.KEYCODE_LEFT_BRACKET: return 'BracketLeft';
      case KeyCode.KEYCODE_RIGHT_BRACKET: return 'BracketRight';
      case KeyCode.KEYCODE_BACKSLASH: return 'Backslash';
      case KeyCode.KEYCODE_SEMICOLON: return 'Semicolon';
      case KeyCode.KEYCODE_APOSTROPHE: return 'Quote';
      case KeyCode.KEYCODE_GRAVE: return 'Backquote';
      case KeyCode.KEYCODE_COMMA: return 'Comma';
      case KeyCode.KEYCODE_PERIOD: return 'Period';
      case KeyCode.KEYCODE_SLASH: return 'Slash';
      case KeyCode.KEYCODE_CAPS_LOCK: return 'CapsLock';
      case KeyCode.KEYCODE_INSERT: return 'Insert';
      case KeyCode.KEYCODE_NUM_LOCK: return 'NumLock';
      case KeyCode.KEYCODE_SCROLL_LOCK: return 'ScrollLock';
      case KeyCode.KEYCODE_META_LEFT: return 'MetaLeft';
      case KeyCode.KEYCODE_META_RIGHT: return 'MetaRight';
      case KeyCode.KEYCODE_NUMPAD_ADD: return 'NumpadAdd';
      case KeyCode.KEYCODE_NUMPAD_SUBTRACT: return 'NumpadSubtract';
      case KeyCode.KEYCODE_NUMPAD_MULTIPLY: return 'NumpadMultiply';
      case KeyCode.KEYCODE_NUMPAD_DIVIDE: return 'NumpadDivide';
      case KeyCode.KEYCODE_NUMPAD_DOT: return 'NumpadDecimal';
      case KeyCode.KEYCODE_NUMPAD_ENTER: return 'NumpadEnter';
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
}
