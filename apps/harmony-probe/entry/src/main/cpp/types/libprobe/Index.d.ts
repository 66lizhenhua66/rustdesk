export interface ProbeEvent {
  taskId: number;
  kind: 'progress' | 'completed' | 'cancelled';
  step: number;
  total: number;
}

declare const probe: {
  version(): string;
  start(steps: number, interval: number, callback: (event: ProbeEvent) => void): number;
  cancel(taskId: number): boolean;
  dispose(): void;
};

export default probe;
