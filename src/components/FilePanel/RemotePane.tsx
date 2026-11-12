import React from "react";
import { Server, Unplug, Shield } from "lucide-react";
import { AddressBar } from "./AddressBar";
import { ColumnHeader } from "./ColumnHeader";

interface RemotePaneProps {
  onConnect?: () => void;
  isActivePane?: boolean;
  onFocusPane?: () => void;
}

export const RemotePane: React.FC<RemotePaneProps> = ({
  onConnect,
  isActivePane = false,
  onFocusPane,
}) => {
  return (
    <section
      role="region"
      aria-label="Remote Filesystem"
      onClick={onFocusPane}
      className={`flex flex-col h-full rounded border overflow-hidden bg-commander-surface transition-colors ${
        isActivePane ? "border-commander-accent/50" : "border-commander-border"
      }`}
    >
      {/* Pane Title Bar */}
      <div className="flex items-center justify-between px-3 py-1.5 bg-commander-darker border-b border-commander-border text-xs font-medium select-none">
        <div className="flex items-center gap-1.5 text-commander-subtext">
          <Server className="w-3.5 h-3.5" />
          <span>Remote Filesystem</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="flex items-center gap-1 text-[10px] text-amber-400 bg-amber-950/40 px-1.5 py-0.5 rounded border border-amber-800/50">
            <Unplug className="w-3 h-3" />
            <span>Disconnected</span>
          </span>
        </div>
      </div>

      {/* Address & Navigation Bar (Honest Disconnected State) */}
      <AddressBar
        path="sftp://[not connected]/"
        onNavigate={() => {}}
        disabled={true}
        canGoBack={false}
        canGoForward={false}
        canGoUp={false}
      />

      {/* Column Header matching the specification order: Name, Size, Changed, Rights, Owner */}
      <ColumnHeader disabled={true} />

      {/* Disconnected Watermark / Placeholder Banner */}
      <div className="flex-1 flex flex-col items-center justify-center p-6 text-center text-commander-subtext select-none bg-commander-surface/40">
        <div className="w-16 h-16 rounded-full bg-commander-dark border border-commander-border flex items-center justify-center mb-3">
          <Server className="w-8 h-8 opacity-40 text-commander-accent" />
        </div>
        <h3 className="text-sm font-medium text-commander-text mb-1">
          No Remote Connection Established
        </h3>
        <p className="text-xs text-commander-subtext/70 max-w-sm mb-4">
          Connect to an SSH/SFTP server using strict host-key verification and secure Linux Secret
          Service credentials to browse remote files.
        </p>

        <button
          type="button"
          onClick={onConnect}
          className="flex items-center gap-2 px-3.5 py-1.5 rounded bg-commander-accent/20 hover:bg-commander-accent/30 text-commander-accent border border-commander-accent/40 text-xs font-medium transition-colors"
        >
          <Shield className="w-3.5 h-3.5" />
          <span>New Connection...</span>
        </button>
      </div>

      {/* Status Bar */}
      <div className="flex items-center justify-between px-3 py-1 bg-commander-darker border-t border-commander-border text-[11px] text-commander-subtext select-none">
        <span>Disconnected</span>
        <span className="text-[10px] opacity-70">Strict Host-Key Enforced</span>
      </div>
    </section>
  );
};
