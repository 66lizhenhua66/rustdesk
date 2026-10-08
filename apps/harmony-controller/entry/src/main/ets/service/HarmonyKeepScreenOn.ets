import { UIContext } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';
import { KeepScreenOnSetter, SessionKeepScreenOn } from './SessionKeepScreenOn';

export function createSessionKeepScreenOn(ui: UIContext): SessionKeepScreenOn {
  return new SessionKeepScreenOn(async (): Promise<KeepScreenOnSetter> => {
    const context = ui.getHostContext() as common.UIAbilityContext;
    if (!context) { throw new Error('Host context unavailable'); }
    const owner = await context.windowStage.getMainWindow();
    return (enabled: boolean): Promise<void> => owner.setWindowKeepScreenOn(enabled);
  });
}
