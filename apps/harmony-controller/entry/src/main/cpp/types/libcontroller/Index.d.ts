export interface ControllerEvent {
  taskId: number;
  state: string;
  code: string;
  message: string;
  verified: boolean;
  authenticated: boolean;
  authorized: boolean;
  confirmationCode?: string;
  x?: number;
  y?: number;
  textLength?: number;
}

declare const controller: {
  version(): string;
  validateProfile(json: string): string;
  probeEndpoint(endpoint: string, timeoutMs: number, callback: (event: ControllerEvent) => void): number;
  authenticate(requestJson: string, password: string, timeoutMs: number, callback: (event: ControllerEvent) => void): number;
  connectDemo(requestJson: string, password: string, timeoutMs: number, callback: (event: ControllerEvent) => void): number;
  sendPointer(taskId: number, x: number, y: number): boolean;
  sendText(taskId: number, text: string): boolean;
  cancel(taskId: number): boolean;
  dispose(): void;
};

export default controller;
