import React from "react";
import { Plus, X, Loader2 } from "lucide-react";
import { RemoteTabState } from "../../types/tabs";

export interface RemoteTabBarProps {
  tabs: RemoteTabState[];
  activeTabId: string;
  onSelectTab: (tabId: string) => void;
  onCloseTab: (tabId: string) => void;
  onAddTab: () => void;
}

export const RemoteTabBar: React.FC<RemoteTabBarProps> = ({
  tabs,
  activeTabId,
  onSelectTab,
  onCloseTab,
  onAddTab,
}) => {
  return (
    <div
      role="tablist"
      aria-label="Remote Connection Tabs"
      className="flex items-center bg-commander-darker border border-commander-border rounded-t px-1.5 pt-1 gap-1 overflow-x-auto select-none min-h-[32px] shrink-0"
    >
      <div className="flex items-center gap-1 min-w-0 flex-1 overflow-x-auto no-scrollbar">
        {tabs.map((tab) => {
          const isActive = tab.id === activeTabId;
          const statusKind = tab.sessionState.status.status;
          const isConnected = statusKind === "Connected" && Boolean(tab.sessionState.session);
          const isConnecting = statusKind === "Connecting" || statusKind === "Reconnecting";
          const isFailed = statusKind === "Failed";

          return (
            <div
              key={tab.id}
              role="tab"
              aria-selected={isActive}
              tabIndex={isActive ? 0 : -1}
              onClick={() => onSelectTab(tab.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  onSelectTab(tab.id);
                }
              }}
              className={`group flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-t border-t border-l border-r font-medium cursor-pointer transition-colors max-w-[220px] shrink-0 ${
                isActive
                  ? "bg-commander-surface border-commander-border text-commander-text border-b-transparent shadow-sm font-semibold"
                  : "bg-commander-dark/50 hover:bg-commander-dark border-transparent text-commander-subtext hover:text-commander-text"
              }`}
              title={`${tab.title} (${statusKind})\nRemote: ${tab.currentPath || "Not connected"}\nLocal: ${tab.localPath || "Default"}`}
            >
              {/* Tab Connection Status Indicator */}
              {isConnecting ? (
                <Loader2 className="w-3 h-3 text-sky-400 animate-spin shrink-0" />
              ) : isConnected ? (
                <span className="w-2 h-2 rounded-full bg-emerald-400 shrink-0" />
              ) : isFailed ? (
                <span className="w-2 h-2 rounded-full bg-rose-400 shrink-0" />
              ) : (
                <span className="w-2 h-2 rounded-full bg-commander-subtext/40 shrink-0" />
              )}

              {/* Tab Title */}
              <span className="truncate text-[11px] leading-tight flex-1">{tab.title}</span>

              {/* Close Tab Button */}
              {tabs.length > 1 && (
                <button
                  type="button"
                  aria-label={`Close tab ${tab.title}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    onCloseTab(tab.id);
                  }}
                  className="p-0.5 rounded hover:bg-rose-950/80 hover:text-rose-200 text-commander-subtext/60 group-hover:text-commander-subtext transition-colors shrink-0 ml-1"
                  title="Close Tab"
                >
                  <X className="w-3 h-3" />
                </button>
              )}
            </div>
          );
        })}
      </div>

      {/* Add New Tab Button */}
      <button
        type="button"
        aria-label="New connection tab"
        onClick={onAddTab}
        className="flex items-center justify-center p-1 rounded hover:bg-commander-dark text-commander-subtext hover:text-commander-text transition-colors shrink-0"
        title="Open New Remote Tab"
      >
        <Plus className="w-3.5 h-3.5" />
      </button>
    </div>
  );
};
