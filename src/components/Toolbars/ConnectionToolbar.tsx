import React from "react";
import {
  Server,
  Unplug,
  RefreshCw,
  ShieldCheck,
  ShieldAlert,
  Terminal,
  SlidersHorizontal,
} from "lucide-react";
import { ConnectionSession } from "../../types/session";

interface ConnectionToolbarProps {
  isConnected: boolean;
  isConnecting?: boolean;
  canReconnect?: boolean;
  canOpenTerminal?: boolean;
  activeSession?: ConnectionSession | null;
  onSiteManager?: () => void;
  onDisconnect?: () => void;
  onReconnect?: () => void;
  onOpenTerminal?: () => void;
  onOpenTerminalSettings?: () => void;
  hostKeyVerified?: boolean;
}

export const ConnectionToolbar: React.FC<ConnectionToolbarProps> = ({
  isConnected,
  isConnecting = false,
  canReconnect = false,
  canOpenTerminal = false,
  activeSession = null,
  onSiteManager,
  onDisconnect,
  onReconnect,
  onOpenTerminal,
  onOpenTerminalSettings,
  hostKeyVerified = true,
}) => {
  const canDisconnectOrCancel = isConnected || isConnecting;
  const terminalEnabled = Boolean(canOpenTerminal || (isConnected && activeSession?.profile_id));

  return (
    <div
      role="toolbar"
      aria-label="Connection Toolbar"
      className="flex items-center justify-between px-3 py-1.5 bg-commander-surface/90 border-b border-commander-border text-xs select-none"
    >
      <div className="flex items-center gap-1.5">
        <button
          type="button"
          onClick={onSiteManager}
          className="flex items-center gap-1.5 px-2.5 py-1 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-text hover:text-white transition-colors"
          title="Open saved site profiles"
        >
          <Server className="w-3.5 h-3.5 text-commander-accent" />
          <span>Site Manager</span>
        </button>

        {onReconnect && (
          <button
            type="button"
            onClick={onReconnect}
            disabled={!canReconnect || isConnecting}
            className={`flex items-center gap-1.5 px-2.5 py-1 rounded border transition-colors ${
              canReconnect && !isConnecting
                ? "bg-commander-dark hover:bg-commander-selected border-commander-border text-commander-text hover:text-white cursor-pointer"
                : "bg-commander-dark/50 border-commander-border/40 text-commander-subtext/40 cursor-not-allowed"
            }`}
            title="Reconnect using the last active profile"
          >
            <RefreshCw
              className={`w-3.5 h-3.5 text-commander-accent ${isConnecting ? "animate-spin" : ""}`}
            />
            <span>Reconnect</span>
          </button>
        )}

        <button
          type="button"
          onClick={onDisconnect}
          disabled={!canDisconnectOrCancel}
          className={`flex items-center gap-1.5 px-2.5 py-1 rounded border transition-colors ${
            canDisconnectOrCancel
              ? "bg-commander-dark hover:bg-rose-950/40 border-commander-border hover:border-rose-800 text-rose-300 cursor-pointer"
              : "bg-commander-dark/50 border-commander-border/40 text-commander-subtext/40 cursor-not-allowed"
          }`}
          title={
            isConnected
              ? "Disconnect active session"
              : isConnecting
                ? "Cancel in-progress connection"
                : "No active session to disconnect"
          }
        >
          <Unplug className="w-3.5 h-3.5" />
          <span>Disconnect</span>
        </button>

        <div className="h-4 w-px bg-commander-border mx-0.5" />

        <button
          type="button"
          onClick={onOpenTerminal}
          disabled={!terminalEnabled}
          className={`flex items-center gap-1.5 px-2.5 py-1 rounded border transition-colors ${
            terminalEnabled
              ? "bg-commander-dark hover:bg-commander-selected border-commander-border text-commander-text hover:text-white cursor-pointer"
              : "bg-commander-dark/50 border-commander-border/40 text-commander-subtext/40 cursor-not-allowed"
          }`}
          title={
            terminalEnabled
              ? "Launch an external interactive SSH terminal for the selected connection"
              : "Connect to or select a saved SSH/SFTP profile to open an external terminal"
          }
        >
          <Terminal className="w-3.5 h-3.5 text-commander-accent" />
          <span>Open Terminal</span>
        </button>

        {onOpenTerminalSettings && (
          <button
            type="button"
            onClick={onOpenTerminalSettings}
            className="flex items-center justify-center p-1 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-subtext hover:text-white transition-colors"
            title="Configure external SSH terminal emulator"
            aria-label="Terminal Settings"
          >
            <SlidersHorizontal className="w-3.5 h-3.5" />
          </button>
        )}
      </div>

      <div className="flex items-center gap-3 text-[11px] text-commander-subtext">
        <div className="flex items-center gap-1">
          {hostKeyVerified ? (
            <ShieldCheck className="w-3.5 h-3.5 text-emerald-400" />
          ) : (
            <ShieldAlert className="w-3.5 h-3.5 text-amber-400" />
          )}
          <span>Strict Host-Key Verification</span>
        </div>
        <div className="h-3 w-px bg-commander-border" />
        <div className="flex items-center gap-1.5">
          <span
            className={`w-2 h-2 rounded-full ${
              isConnected
                ? "bg-emerald-500"
                : isConnecting
                  ? "bg-sky-400 animate-pulse"
                  : "bg-commander-subtext/40"
            }`}
          />
          <span>
            {isConnected
              ? activeSession
                ? `Remote Connected (${activeSession.username}@${activeSession.host}:${activeSession.port})`
                : "Remote Connected"
              : isConnecting
                ? "Connecting..."
                : "Remote Disconnected"}
          </span>
        </div>
      </div>
    </div>
  );
};
