import { MonitorIcon } from "@phosphor-icons/react";

import { SectionCard, Switch } from "@/components";
import type { Settings } from "@/lib/tauri";

import { SettingGroup } from "./SettingGroup";
import { SettingRow } from "./SettingRow";

interface StartupAndTraySectionProps {
  settings: Settings;
  onSave: (settings: Settings) => void;
}

export function StartupAndTraySection({ settings, onSave }: StartupAndTraySectionProps) {
  return (
    <SectionCard title="Startup and tray" icon={<MonitorIcon className="h-5 w-5" />}>
      <SettingGroup id="general.startup" title="Startup">
        <SettingRow
          setting="autoRun"
          description="Automatically launch VinciSkin when you start your computer."
          control={
            <Switch
              checked={settings.autoRun}
              onCheckedChange={(checked) => onSave({ ...settings, autoRun: checked })}
            />
          }
        />
      </SettingGroup>

      <SettingGroup id="general.tray" title="Tray">
        <SettingRow
          setting="minimizeToTray"
          description="Minimizing hides the window to the tray instead of the taskbar. Click the tray icon to restore it."
          control={
            <Switch
              checked={settings.minimizeToTray}
              onCheckedChange={(checked) => onSave({ ...settings, minimizeToTray: checked })}
            />
          }
        />

        <SettingRow
          setting="startInTray"
          description="The app starts hidden in the tray. Click the tray icon to open it."
          control={
            <Switch
              checked={settings.startInTray}
              onCheckedChange={(checked) => onSave({ ...settings, startInTray: checked })}
            />
          }
        />
      </SettingGroup>
    </SectionCard>
  );
}
