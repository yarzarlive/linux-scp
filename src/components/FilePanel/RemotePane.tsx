import React from "react";
import {
  Server,
  Unplug,
  Shield,
  ShieldCheck,
  RefreshCw,
  Loader2,
  AlertTriangle,
  CheckCircle2,
  FolderCheck,
  KeyRound,
} from "lucide-react";
import { AddressBar } from "./AddressBar";
import { ColumnHeader } from "./ColumnHeader";
import { SessionStateSnapshot } from "../../types/session";

interface RemotePaneProps {
  sessionState?: SessionStateSnapshot;
  connectionError?: string | null;
  onConnect?: () => void;
  onDisconnect?: () => void;
  onReconnect?: () => void;
  isActivePane?: boolean;
  onFocusPane?: () => void;
}

const DEFAULT_SESSION_STATE: SessionStateSnapshot = {
  status: { status: "Disconnected" },
  session: null,
  trusted_host_key: null,
};

export const RemotePane: React.FC<RemotePaneProps> = ({
  sessionState = DEFAULT_SESSION_STATE,
  connectionError = null,
  onConnect,
  onDisconnect,
  onReconnect,
  isActivePane = false,
  onFocusPane,
}) => {
  const statusKind = sessionState.status.status;
  const activeSession = statusKind === "Connected" ? sessionState.session : null;
  const trustedHostKey = sessionState.trusted_host_key;

  const isConnecting = statusKind === "Connecting" || statusKind === "Reconnecting";
  const isFailed = statusKind === "Failed";
  const isConnected = statusKind === "Connected" && Boolean(activeSession);

  const errorMessage =
    connectionError ||
    (sessionState.status.status === "Failed" ? sessionState.status.details.error_message : null);

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
          {activeSession && (
            <span className="ml-1 font-mono text-[11px] text-commander-text">
              ({activeSession.username}@{activeSession.host}:{activeSession.port})
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {isConnected && activeSession ? (
            <span className="flex items-center gap-1 text-[10px] text-emerald-400 bg-emerald-950/40 px-1.5 py-0.5 rounded border border-emerald-800/50">
              <CheckCircle2 className="w-3 h-3" />
              <span>Connected</span>
            </span>
          ) : isConnecting ? (
            <span className="flex items-center gap-1 text-[10px] text-sky-400 bg-sky-950/40 px-1.5 py-0.5 rounded border border-sky-800/50">
              <Loader2 className="w-3 h-3 animate-spin" />
              <span>{statusKind === "Reconnecting" ? "Reconnecting" : "Connecting"}</span>
            </span>
          ) : isFailed ? (
            <span className="flex items-center gap-1 text-[10px] text-rose-400 bg-rose-950/40 px-1.5 py-0.5 rounded border border-rose-800/50">
              <AlertTriangle className="w-3 h-3" />
              <span>Failed</span>
            </span>
          ) : (
            <span className="flex items-center gap-1 text-[10px] text-amber-400 bg-amber-950/40 px-1.5 py-0.5 rounded border border-amber-800/50">
              <Unplug className="w-3 h-3" />
              <span>Disconnected</span>
            </span>
          )}
        </div>
      </div>

      {/* Address & Navigation Bar */}
      <AddressBar
        path={
          isConnected && activeSession
            ? activeSession.current_remote_path
            : "sftp://[not connected]/"
        }
        onNavigate={() => {}}
        disabled={true}
        canGoBack={false}
        canGoForward={false}
        canGoUp={false}
      />

      {/* Column Header matching the specification order: Name, Size, Changed, Rights, Owner */}
      <ColumnHeader disabled={true} />

      {/* Main Content Area by Lifecycle State */}
      {isConnecting ? (
        <div className="flex-1 flex flex-col items-center justify-center p-6 text-center text-commander-subtext select-none bg-commander-surface/40">
          <div className="w-16 h-16 rounded-full bg-commander-dark border border-sky-800/60 flex items-center justify-center mb-3">
            <Loader2 className="w-8 h-8 animate-spin text-sky-400" />
          </div>
          <h3 className="text-sm font-medium text-commander-text mb-1">
            {statusKind === "Reconnecting"
              ? "Reconnecting SFTP Session..."
              : "Establishing SFTP Session..."}
          </h3>
          <p className="text-xs text-commander-subtext/80 max-w-sm mb-4">
            Verifying SSH server host key, authenticating credentials, and initializing the remote
            SFTP subsystem...
          </p>
          {onDisconnect && (
            <button
              type="button"
              onClick={onDisconnect}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded bg-rose-950/40 hover:bg-rose-950/60 text-rose-300 border border-rose-800/60 text-xs font-medium transition-colors"
            >
              <Unplug className="w-3.5 h-3.5" />
              <span>Cancel Connection</span>
            </button>
          )}
        </div>
      ) : isConnected && activeSession ? (
        <div className="flex-1 flex flex-col items-center justify-center p-6 text-commander-text select-none bg-commander-surface/40 overflow-y-auto">
          <div className="w-full max-w-md rounded-lg border border-emerald-800/50 bg-commander-dark/90 p-4 shadow-lg space-y-3">
            <div className="flex items-center justify-between border-b border-commander-border pb-2.5">
              <div className="flex items-center gap-2">
                <ShieldCheck className="w-5 h-5 text-emerald-400 shrink-0" />
                <div>
                  <h3 className="text-xs font-semibold text-emerald-300">
                    SFTP Subsystem Established &amp; Ready
                  </h3>
                  <p className="text-[11px] text-commander-subtext font-mono">
                    {activeSession.username}@{activeSession.host}:{activeSession.port}
                  </p>
                </div>
              </div>
              <span className="text-[10px] font-mono uppercase px-2 py-0.5 rounded bg-emerald-950/60 text-emerald-300 border border-emerald-800/50">
                {activeSession.protocol}
              </span>
            </div>

            <div className="space-y-2 text-xs font-mono">
              <div className="flex items-center justify-between py-1 border-b border-commander-border/40">
                <span className="text-commander-subtext flex items-center gap-1.5 font-sans">
                  <FolderCheck className="w-3.5 h-3.5 text-commander-accent" />
                  Verified Working Dir:
                </span>
                <span className="text-commander-text select-text">
                  {activeSession.current_remote_path}
                </span>
              </div>

              <div className="flex items-center justify-between py-1 border-b border-commander-border/40">
                <span className="text-commander-subtext font-sans">Elevation Mode:</span>
                <span className="text-commander-text">
                  {activeSession.elevation.type === "ElevatedSudo"
                    ? `Elevated (${activeSession.elevation.effective_user})`
                    : "Standard User"}
                </span>
              </div>

              {trustedHostKey && (
                <div className="pt-1 space-y-1">
                  <div className="flex items-center justify-between">
                    <span className="text-commander-subtext flex items-center gap-1.5 font-sans">
                      <KeyRound className="w-3.5 h-3.5 text-emerald-400" />
                      Host Key ({trustedHostKey.algorithm}):
                    </span>
                  </div>
                  <div className="p-2 rounded bg-commander-darker border border-commander-border text-[10px] text-emerald-400 break-all select-text">
                    {trustedHostKey.fingerprint_sha256}
                  </div>
                </div>
              )}
            </div>

            <div className="flex items-center justify-end gap-2 pt-2 border-t border-commander-border">
              {onReconnect && (
                <button
                  type="button"
                  onClick={onReconnect}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded bg-commander-surface hover:bg-commander-selected border border-commander-border text-commander-text text-xs transition-colors"
                >
                  <RefreshCw className="w-3.5 h-3.5 text-commander-accent" />
                  <span>Reconnect</span>
                </button>
              )}
              {onDisconnect && (
                <button
                  type="button"
                  onClick={onDisconnect}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded bg-rose-950/40 hover:bg-rose-950/70 border border-rose-800/60 text-rose-300 text-xs transition-colors"
                >
                  <Unplug className="w-3.5 h-3.5" />
                  <span>Disconnect</span>
                </button>
              )}
            </div>
          </div>
        </div>
      ) : isFailed ? (
        <div className="flex-1 flex flex-col items-center justify-center p-6 text-center text-commander-subtext select-none bg-commander-surface/40">
          <div className="w-14 h-14 rounded-full bg-rose-950/40 border border-rose-800/60 flex items-center justify-center mb-3">
            <AlertTriangle className="w-7 h-7 text-rose-400" />
          </div>
          <h3 className="text-sm font-medium text-rose-200 mb-1">Remote SFTP Connection Failed</h3>
          <p className="text-xs text-rose-300/90 max-w-md mb-4 p-2.5 rounded bg-rose-950/30 border border-rose-900/50 font-mono break-words select-text">
            {errorMessage || "Unable to establish SFTP session."}
          </p>
          <div className="flex items-center gap-2">
            {onReconnect && (
              <button
                type="button"
                onClick={onReconnect}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-text text-xs font-medium transition-colors"
              >
                <RefreshCw className="w-3.5 h-3.5 text-commander-accent" />
                <span>Retry Connection</span>
              </button>
            )}
            <button
              type="button"
              onClick={onConnect}
              className="flex items-center gap-2 px-3.5 py-1.5 rounded bg-commander-accent/20 hover:bg-commander-accent/30 text-commander-accent border border-commander-accent/40 text-xs font-medium transition-colors"
            >
              <Shield className="w-3.5 h-3.5" />
              <span>New Connection...</span>
            </button>
          </div>
        </div>
      ) : (
        /* Disconnected Watermark / Placeholder Banner */
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
      )}

      {/* Status Bar */}
      <div className="flex items-center justify-between px-3 py-1 bg-commander-darker border-t border-commander-border text-[11px] text-commander-subtext select-none">
        <span>
          {isConnected && activeSession
            ? `Connected • SFTP Subsystem Ready (${activeSession.current_remote_path})`
            : isConnecting
              ? "Handshaking SSH & SFTP..."
              : isFailed
                ? "Connection Failed"
                : "Disconnected"}
        </span>
        <span className="text-[10px] opacity-70">
          {trustedHostKey
            ? `Host Key Verified (${trustedHostKey.algorithm})`
            : "Strict Host-Key Enforced"}
        </span>
      </div>
    </section>
  );
};
