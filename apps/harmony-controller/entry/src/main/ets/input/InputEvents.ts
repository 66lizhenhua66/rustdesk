export interface InputPoint {
  id: number;
  x: number;
  y: number;
}

export interface TouchInputEvent {
  action: string;
  points: InputPoint[];
  changedPoints: InputPoint[];
  time: number;
}

export interface MouseInputEvent {
  action: string;
  button: string;
  x: number;
  y: number;
}

export interface WheelInputEvent {
  action: string;
  x: number;
  y: number;
  dx: number;
  dy: number;
  discrete: boolean;
}

export interface KeyInputEvent {
  action: string;
  physicalCode: number;
  code: string;
}

export interface NativeInputEvent {
  kind: string;
  action?: string;
  points?: InputPoint[];
  changedPoints?: InputPoint[];
  time?: number;
  mode?: string;
  x?: number;
  y?: number;
  dx?: number;
  dy?: number;
  discrete?: boolean;
  button?: string;
  physicalCode?: number;
  code?: string;
  text?: string;
}
