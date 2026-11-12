import React from "react";
import { PlusCircle, Server, Unplug, ShieldCheck, ShieldAlert } from "lucide-react";

interface ConnectionToolbarProps {
  isConnected: boolean;
  onNewSession?: () => void;
  onSiteManager?: () => void;
  onDisconnect?: () => void;
  hostKeyVerified?: boolean;
}

export const ConnectionToolbar: React.FC<ConnectionToolbarProps> = ({
  isConnected,
  onNewSession,
  onSiteManager,
  onDisconnect,
  hostKeyVerified = true,
}) => {
  return (
    <div
      role="toolbar"
      aria-label="Connection Toolbar"
      className="flex items-center justify-between px-3 py-1.5 bg-commander-surface/90 border-b border-commander-border text-xs select-none"
    >
      <div className="flex items-center gap-1.5">
        <button
          type="button"
          onClick={onNewSession}
          className="flex items-center gap-1.5 px-2.5 py-1 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-text hover:text-white transition-colors"
          title="Open a new SSH/SFTP connection dialog"
        >
          <PlusCircle className="w-3.5 h-3.5 text-commander-accent" />
          <span>New Session</span>
        </button>

        <button
          type="button"
          onClick={onSiteManager}
          className="flex items-center gap-1.5 px-2.5 py-1 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-text hover:text-white transition-colors"
          title="Open saved site profiles"
        >
          <Server className="w-3.5 h-3.5 text-commander-accent" />
          <span>Site Manager</span>
        </button>

        <button
          type="button"
          onClick={onDisconnect}
          disabled={!isConnected}
          className={`flex items-center gap-1.5 px-2.5 py-1 rounded border transition-colors ${
            isConnected
              ? "bg-commander-dark hover:bg-rose-950/40 border-commander-border hover:border-rose-800 text-rose-300 cursor-pointer"
              : "bg-commander-dark/50 border-commander-border/40 text-commander-subtext/40 cursor-not-allowed"
          }`}
          title={isConnected ? "Disconnect active session" : "No active session to disconnect"}
        >
          <Unplug className="w-3.5 h-3.5" />
          <span>Disconnect</span>
        </button>
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
              isConnected ? "bg-emerald-500" : "bg-commander-subtext/40"
            }`}
          />
          <span>{isConnected ? "Remote Connected" : "Remote Disconnected"}</span>
        </div>
      </div>
    </div>
  );
};
