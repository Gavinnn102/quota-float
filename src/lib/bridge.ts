import type { ProviderSnapshot, QuotaWindow, WidgetPreferences } from "../types";

const defaultPreferences: WidgetPreferences = { panelVisible: true, expanded: true, alwaysOnTop: true, pinnedProvider: null, autoRotateSeconds: 12, language: "zh-CN", quotaWindow: "weekly" };

const mockSnapshot: ProviderSnapshot = {
  provider: "codex",
  displayName: "CODEX",
  plan: "PRO",
  weeklyWindow: { remainingPercent: 42, resetsAt: new Date(Date.now() + 3.2 * 86_400_000).toISOString(), windowSeconds: 604_800 },
  fiveHourWindow: { remainingPercent: 76, resetsAt: new Date(Date.now() + 3.5 * 3_600_000).toISOString(), windowSeconds: 18_000 },
  credits: { balance: 1211, unlimited: false },
  resetCredits: 1,
  resetCreditExpiresAt: [new Date(Date.now() + 9 * 86_400_000).toISOString()],
  updatedAt: new Date().toISOString(),
  status: "ok",
  message: null,
};

export const isTauri = () => "__TAURI_INTERNALS__" in window;

export async function fetchSnapshots(force = false): Promise<ProviderSnapshot[]> {
  if (!isTauri()) return [mockSnapshot];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ProviderSnapshot[]>(force ? "refresh_snapshots" : "get_snapshots");
}

export async function getPreferences(): Promise<WidgetPreferences> {
  if (!isTauri()) return defaultPreferences;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<WidgetPreferences>("get_preferences");
}

export async function updatePreferences(value: WidgetPreferences): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("set_preferences", { preferences: value });
}

export async function setQuotaWindow(quotaWindow: QuotaWindow): Promise<WidgetPreferences> {
  if (!isTauri()) return { ...defaultPreferences, quotaWindow };
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<WidgetPreferences>("set_quota_window", { quotaWindow });
}

export async function setAlwaysOnTop(alwaysOnTop: boolean): Promise<WidgetPreferences> {
  if (!isTauri()) return { ...defaultPreferences, alwaysOnTop };
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<WidgetPreferences>("set_widget_always_on_top", { alwaysOnTop });
}

export async function setWidgetExpanded(expanded: boolean): Promise<WidgetPreferences> {
  if (!isTauri()) {
    if (expanded) delete document.documentElement.dataset.mockWidgetSize;
    else document.documentElement.dataset.mockWidgetSize = "compact";
    return { ...defaultPreferences, expanded };
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<WidgetPreferences>("set_widget_expanded", { expanded });
}

export async function listenDesktopEvents(handlers: {
  onPreferences: (value: WidgetPreferences) => void;
  onRefresh: () => void;
}): Promise<() => void> {
  if (!isTauri()) return () => undefined;
  const { listen } = await import("@tauri-apps/api/event");
  const unlistenPreferences = await listen<WidgetPreferences>("preferences-changed", (event) => handlers.onPreferences(event.payload));
  const unlistenRefresh = await listen("refresh-requested", handlers.onRefresh);
  return () => { unlistenPreferences(); unlistenRefresh(); };
}
