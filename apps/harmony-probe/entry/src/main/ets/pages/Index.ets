import probe, { ProbeEvent } from 'libprobe.so';
import { hilog } from '@kit.PerformanceAnalysisKit';

@Entry
@Component
struct Index {
  @State coreVersion: string = '';
  @State status: string = '准备验证';
  @State progress: number = 0;
  @State report: string = '';
  @State running: boolean = false;
  private currentTask: number = 0;
  private epoch: number = 0;
  private alive: boolean = true;

  aboutToAppear(): void {
    this.alive = true;
    this.coreVersion = probe.version();
  }

  aboutToDisappear(): void {
    this.stopPage();
  }

  onPageHide(): void {
    this.stopPage();
  }

  onPageShow(): void {
    this.alive = true;
    this.runChecks();
  }

  private stopPage(): void {
    this.alive = false;
    this.epoch++;
    probe.dispose();
    this.running = false;
  }

  private record(message: string): void {
    if (!this.alive) { return; }
    this.report = `${this.report}${message}\n`;
    hilog.info(0x0000, 'BridgeProbe', '%{public}s', message);
  }

  private completionCheck(ownerEpoch: number): Promise<boolean> {
    return new Promise<boolean>((resolve: (value: boolean) => void) => {
      let count: number = 0;
      let settled: boolean = false;
      const timeoutId = setTimeout(() => {
        if (!settled) {
          settled = true;
          if (this.alive && ownerEpoch === this.epoch) { probe.dispose(); }
          resolve(false);
        }
      }, 3000);
      this.currentTask = probe.start(3, 20, (event: ProbeEvent) => {
        if (!this.alive || ownerEpoch !== this.epoch || event.taskId !== this.currentTask || settled) { return; }
        if (event.kind === 'progress') { count++; }
        if (event.kind === 'completed') {
          settled = true;
          clearTimeout(timeoutId);
          resolve(count === 3 && event.step === 3 && event.total === 3);
        }
      });
    });
  }

  private cancellationCheck(ownerEpoch: number): Promise<boolean> {
    return new Promise<boolean>((resolve: (value: boolean) => void) => {
      let count: number = 0;
      let cancelled: boolean = false;
      let settled: boolean = false;
      const timeoutId = setTimeout(() => {
        if (!settled) {
          settled = true;
          if (this.alive && ownerEpoch === this.epoch) { probe.dispose(); }
          resolve(false);
        }
      }, 3000);
      this.currentTask = probe.start(100, 20, (event: ProbeEvent) => {
        if (!this.alive || ownerEpoch !== this.epoch || event.taskId !== this.currentTask || settled) { return; }
        if (event.kind === 'progress') {
          count++;
          if (!cancelled) {
            cancelled = probe.cancel(event.taskId);
            setTimeout(() => {
              if (!settled) {
                settled = true;
                clearTimeout(timeoutId);
                resolve(this.alive && ownerEpoch === this.epoch && cancelled && count === 1);
              }
            }, 150);
          }
        }
      });
    });
  }

  private replacementCheck(ownerEpoch: number): Promise<boolean> {
    return new Promise<boolean>((resolve: (value: boolean) => void) => {
      let stale: number = 0;
      let settled: boolean = false;
      const timeoutId = setTimeout(() => {
        if (!settled) {
          settled = true;
          if (this.alive && ownerEpoch === this.epoch) { probe.dispose(); }
          resolve(false);
        }
      }, 3000);
      probe.start(100, 500, (_event: ProbeEvent) => { stale++; });
      this.currentTask = probe.start(2, 20, (event: ProbeEvent) => {
        if (!this.alive || ownerEpoch !== this.epoch || event.taskId !== this.currentTask || settled) { return; }
        if (event.kind === 'completed') {
          settled = true;
          clearTimeout(timeoutId);
          resolve(stale === 0 && event.step === 2);
        }
      });
    });
  }

  private async runChecks(): Promise<void> {
    if (this.running) { return; }
    const ownEpoch = ++this.epoch;
    this.running = true;
    this.report = '';
    this.status = '自动验证中';
    let passed: number = 0;
    try {
      if (this.coreVersion.length > 0) { passed++; this.record('PASS version'); }
      let rejected: boolean = false;
      try { probe.start(0, 20, (_event: ProbeEvent) => {}); } catch { rejected = true; }
      if (rejected) { passed++; this.record('PASS invalid_arguments'); }
      const completed = await this.completionCheck(ownEpoch);
      if (!this.alive || ownEpoch !== this.epoch) { return; }
      if (completed) { passed++; this.record('PASS async_completion'); }
      const cancelled = await this.cancellationCheck(ownEpoch);
      if (!this.alive || ownEpoch !== this.epoch) { return; }
      if (cancelled) { passed++; this.record('PASS cancel_no_late_progress'); }
      const replaced = await this.replacementCheck(ownEpoch);
      if (!this.alive || ownEpoch !== this.epoch) { return; }
      if (replaced) { passed++; this.record('PASS replace_no_stale_callback'); }
      probe.dispose();
      probe.dispose();
      const restarted = await this.completionCheck(ownEpoch);
      if (!this.alive || ownEpoch !== this.epoch) { return; }
      if (restarted) { passed++; this.record('PASS dispose_and_restart'); }
      probe.dispose();
      this.status = passed === 6 ? '自动验证通过 6/6' : `验证未通过 ${passed}/6`;
      this.record(`RESULT ${passed}/6`);
    } catch {
      if (!this.alive || ownEpoch !== this.epoch) { return; }
      this.status = '验证异常，请查看日志';
      this.record('RESULT ERROR');
      probe.dispose();
    } finally {
      if (this.alive && ownEpoch === this.epoch) { this.running = false; }
    }
  }

  private startManual(): void {
    this.epoch++;
    this.progress = 0;
    this.status = '后台任务运行中，界面仍可操作';
    this.currentTask = probe.start(100, 100, (event: ProbeEvent) => {
      if (!this.alive || event.taskId !== this.currentTask) { return; }
      this.progress = event.step;
      if (event.kind === 'completed') { this.status = '后台任务完成'; }
    });
  }

  build() {
    Scroll() {
      Column({ space: 18 }) {
        Text('鸿蒙原生桥接验证').fontSize(26).fontWeight(FontWeight.Bold)
        Text('ArkUI → NAPI → Rust').fontSize(18).fontColor('#176BDF')
        Text(this.coreVersion).fontSize(13).fontColor('#64748B')
        Text(this.status).fontSize(18).fontWeight(FontWeight.Medium)
        Progress({ value: this.progress, total: 100, type: ProgressType.Linear }).width('100%')
        Button('重新运行自动验证').enabled(!this.running).onClick(() => { this.runChecks(); })
        Button('开始后台任务').enabled(!this.running).onClick(() => { this.startManual(); })
        Button('取消任务').enabled(!this.running).onClick(() => {
          probe.cancel(this.currentTask);
          this.status = '任务已取消';
        })
        Button('释放资源').enabled(!this.running).onClick(() => {
          this.epoch++;
          probe.dispose();
          this.status = '资源已释放，可重新开始';
        })
        Text(this.report).fontSize(13).fontFamily('monospace').width('100%')
        Text('此工程只验证桥接与生命周期，不建立远程连接。')
          .fontSize(13).fontColor('#64748B').width('100%')
      }.padding(24).width('100%')
    }.width('100%').height('100%').backgroundColor('#F4F6FA')
  }
}
