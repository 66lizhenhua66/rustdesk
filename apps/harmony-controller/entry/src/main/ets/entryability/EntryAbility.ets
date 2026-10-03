import { UIAbility } from '@kit.AbilityKit';
import { window } from '@kit.ArkUI';
import { hilog } from '@kit.PerformanceAnalysisKit';
import controller from 'libcontroller.so';

export default class EntryAbility extends UIAbility {
  onWindowStageCreate(stage: window.WindowStage): void {
    try {
      stage.loadContent('pages/Desk', (error) => {
        if (error.code) {
          hilog.error(0x0000, 'Controller', 'loadContent failed %{public}d', error.code);
        }
      });
    } catch {
      hilog.error(0x0000, 'Controller', 'loadContent threw');
    }
  }

  onForeground(): void {
    AppStorage.setOrCreate('deskForeground', true);
  }

  onBackground(): void {
    AppStorage.setOrCreate('deskForeground', false);
    controller.dispose();
  }

  onDestroy(): void {
    AppStorage.setOrCreate('deskForeground', false);
    controller.dispose();
  }
}
