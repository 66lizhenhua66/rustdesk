export type KeepScreenOnSetter = (enabled: boolean) => Promise<void>;

export class SessionKeepScreenOn {
  private acquire: () => Promise<KeepScreenOnSetter>;
  private setter?: KeepScreenOnSetter;
  private wanted: boolean = false;
  private applied: boolean = false;
  private applying: boolean = false;
  private stopped: boolean = false;

  constructor(acquire: () => Promise<KeepScreenOnSetter>) {
    this.acquire = acquire;
  }

  setEnabled(enabled: boolean): void {
    const wanted = enabled && !this.stopped;
    if (this.wanted === wanted) { return; }
    this.wanted = wanted;
    this.apply();
  }

  stop(): void {
    this.stopped = true;
    this.setEnabled(false);
  }

  private async apply(): Promise<void> {
    if (this.applying) { return; }
    this.applying = true;
    try {
      if (!this.setter) { this.setter = await this.acquire(); }
      // Recheck after window acquisition, and serialize changes across native promises.
      while (this.wanted !== this.applied) {
        const enabled = this.wanted;
        await this.setter(enabled);
        this.applied = enabled;
      }
    } catch {
      console.warn('RemoteDesk: failed to update session screen-on state.');
    } finally {
      this.applying = false;
    }
  }
}
