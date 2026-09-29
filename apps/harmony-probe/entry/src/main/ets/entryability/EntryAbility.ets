import { UIAbility } from '@kit.AbilityKit';
import { window } from '@kit.ArkUI';
import { hilog } from '@kit.PerformanceAnalysisKit';
import probe from 'libprobe.so';

export default class EntryAbility extends UIAbility {
  onWindowStageCreate(stage: window.WindowStage): void {
    try {
      stage.loadContent('pages/Index', (error) => {
        if (error.code) { hilog.error(0x0000, 'BridgeProbe', 'loadContent failed %{public}d', error.code); }
      });
    } catch {
      hilog.error(0x0000, 'BridgeProbe', 'loadContent threw');
    }
  }

  onBackground(): void {
    probe.dispose();
  }

  onDestroy(): void {
    probe.dispose();
  }
}
