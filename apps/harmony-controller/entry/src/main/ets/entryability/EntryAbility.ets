import { UIAbility } from '@kit.AbilityKit';
import { window } from '@kit.ArkUI';
import { hilog } from '@kit.PerformanceAnalysisKit';
import controller from 'libcontroller.so';

export default class EntryAbility extends UIAbility {
  onWindowStageCreate(stage: window.WindowStage): void {
    try {
      stage.loadContent('pages/Index', (error) => {
        if (error.code) {
          hilog.error(0x0000, 'Controller', 'loadContent failed %{public}d', error.code);
        }
      });
    } catch {
      hilog.error(0x0000, 'Controller', 'loadContent threw');
    }
  }

  onBackground(): void {
    controller.dispose();
  }

  onDestroy(): void {
    controller.dispose();
  }
}
