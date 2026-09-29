export interface ControllerEvent {
  taskId: number;
  state: string;
  code: string;
  message: string;
  verified: boolean;
  authorized: boolean;
}

declare const controller: {
  version(): string;
  validateProfile(json: string): string;
  probeEndpoint(endpoint: string, timeoutMs: number, callback: (event: ControllerEvent) => void): number;
  cancel(taskId: number): boolean;
  dispose(): void;
};

export default controller;
