export interface ControllerEvent {
  taskId: number;
  state: string;
  code: string;
  message: string;
  verified: boolean;
  authenticated: boolean;
  authorized: boolean;
  inputSupported?: boolean;
  confirmationCode?: string;
  x?: number;
  y?: number;
  textLength?: number;
  videoWidth?: number;
  videoHeight?: number;
  videoCodec?: string;
  videoQuality?: string;
  videoFps?: number;
  videoSettingsSupported?: boolean;
  videoRequestId?: number;
  videoSettingsVersion?: number;
  videoResolutionMode?: string;
  videoResolutionWidth?: number;
  videoResolutionHeight?: number;
  desktopWidth?: number;
  desktopHeight?: number;
  originalWidth?: number;
  originalHeight?: number;
  supportedResolutions?: { width: number; height: number }[];
  resolutionSyncSupported?: boolean;
  videoSettingsError?: string;
  frames?: number;
  bytes?: number;
  renderedFrames?: number;
  connectionPath?: string;
  accessMode?: string;
  credentialId?: string;
  credentialSecret?: string;
  expiresAt?: number;
}

declare const controller: {
  version(): string;
  createIdentity(): string;
  validateProfile(json: string): string;
  probeEndpoint(endpoint: string, timeoutMs: number, callback: (event: ControllerEvent) => void): number;
  authenticate(requestJson: string, password: string, timeoutMs: number, callback: (event: ControllerEvent) => void): number;
  connectDemo(requestJson: string, password: string, timeoutMs: number, callback: (event: ControllerEvent) => void): number;
  connectScreen(requestJson: string, surfaceId: string, timeoutMs: number, callback: (event: ControllerEvent) => void): number;
  connectTrustedScreen(requestJson: string, credentialsJson: string, surfaceId: string, timeoutMs: number,
    callback: (event: ControllerEvent) => void): number;
  sendPointer(taskId: number, x: number, y: number): boolean;
  sendText(taskId: number, text: string): boolean;
  sendInput(taskId: number, commandJson: string): number;
  sendInputEvent(taskId: number, eventJson: string): number;
  resetInput(taskId: number): void;
  canvasState(taskId: number): string;
  setInputEnabled(taskId: number, enabled: boolean): number;
  setVideoSettings(taskId: number, settingsJson: string): number;
  cancel(taskId: number): boolean;
  dispose(): void;
};

export default controller;
