import type { ScreenState } from './DeskModels';

export interface SystemInputCommand {
  kind: string;
  x?: number;
  y?: number;
  button?: string;
  down?: boolean;
  dx?: number;
  dy?: number;
  code?: string;
  text?: string;
}

export class SessionInputMode {
  controlMode: boolean = true;
  requested: boolean = false;
  private accessMode: string = '';

  constructor(controlMode: boolean = true, accessMode: string = '') {
    this.accessMode = accessMode;
    this.controlMode = controlMode && accessMode.length === 0;
  }

  requestIfReady(screen: ScreenState, visible: boolean): boolean {
    if (!this.controlMode || this.accessMode.length > 0 || this.requested ||
      !canRequestSystemInput(screen, visible)) { return false; }
    this.requested = true;
    return true;
  }

  select(controlMode: boolean): void {
    this.controlMode = controlMode && this.accessMode.length === 0;
    this.requested = false;
  }

  settle(screen: ScreenState): void {
    if (this.requested && screen.code === 'INPUT_STATE' && !screen.inputGranted) {
      this.select(false);
    }
  }
}

export function canRequestSystemInput(screen: ScreenState, visible: boolean): boolean {
  return visible && screen.approved && screen.inputSupported &&
    !screen.accessMode && !screen.requestedAccessMode &&
    screen.phase === 'viewing' && screen.rendered > 0;
}

export function canSendSystemInput(screen: ScreenState, visible: boolean, controlMode: boolean): boolean {
  return canRequestSystemInput(screen, visible) && controlMode && screen.inputGranted &&
    !screen.videoSettingsPending &&
    screen.code !== 'INPUT_ENABLING' && screen.code !== 'INPUT_DISABLING';
}
