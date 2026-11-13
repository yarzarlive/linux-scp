import React, { useState } from "react";
import {
  FileEdit,
  Upload,
  AlertTriangle,
  AlertCircle,
  CheckCircle2,
  Trash2,
  Copy,
  RefreshCw,
  ExternalLink,
  Loader2,
  X,
} from "lucide-react";
import { RemoteEditSession, RemoteEditUploadMode } from "../../types/desktop";
import { formatFileSize } from "../../utils/formatters";

interface RemoteEditPanelProps {
  sessions: RemoteEditSession[];
  isConnected: boolean;
  localCurrentDir: string;
  uploadMode?: RemoteEditUploadMode;
  onChangeUploadMode?: (mode: RemoteEditUploadMode) => Promise<void>;
  onUploadSession: (sessionId: string, overwriteRemoteChanges: boolean) => Promise<void>;
  onKeepLocalChanges: (sessionId: string) => Promise<void>;
  onSaveLocalCopy: (sessionId: string, destinationDir: string) => Promise<string | null>;
  onDiscardSession: (sessionId: string, discardLocalChanges: boolean) => Promise<void>;
  onReloadFromRemote?: (sessionId: string) => Promise<void>;
  onDownloadNewerRemote?: (remotePath: string, destinationDir: string) => Promise<void>;
  onChooseAppForRemote?: (remotePath: string, fileName: string) => void;
  onScanNow?: () => Promise<void>;
}

export const RemoteEditPanel: React.FC<RemoteEditPanelProps> = ({
  sessions,
  isConnected,
  localCurrentDir,
  uploadMode = "auto",
  onChangeUploadMode,
  onUploadSession,
  onKeepLocalChanges,
  onSaveLocalCopy,
  onDiscardSession,
  onReloadFromRemote,
  onDownloadNewerRemote,
  onChooseAppForRemote,
  onScanNow,
}) => {
  const [busySessionId, setBusySessionId] = useState<string | null>(null);
  const [confirmDiscardId, setConfirmDiscardId] = useState<string | null>(null);
  const [notice, setNotice] = useState<{
    kind: "info" | "error";
    message: string;
  } | null>(null);

  if (sessions.length === 0) return null;

  const runAction = async (sessionId: string, fn: () => Promise<void>) => {
    setBusySessionId(sessionId);
    setNotice(null);
    try {
      await fn();
    } catch (err: unknown) {
      const msg =
        err && typeof err === "object" && "message" in err
          ? String((err as { message: unknown }).message)
          : String(err ?? "Remote edit action failed");
      setNotice({ kind: "error", message: msg });
    } finally {
      setBusySessionId(null);
    }
  };

  return (
    <section
      aria-label="Remote File Editing Sessions"
      className="border-b border-commander-border bg-commander-darker/95 px-3 py-2 text-xs select-none space-y-2"
    >
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <FileEdit className="w-3.5 h-3.5 text-commander-accent shrink-0" />
          <span className="font-semibold text-commander-text">
            Remote Editing Sessions ({sessions.length})
          </span>
          <span className="text-[11px] text-commander-subtext hidden sm:inline">
            {uploadMode === "auto"
              ? "Auto-uploading local saves when the remote file is unchanged; conflicts are never overwritten automatically."
              : "Prompting before uploading local saves; conflicts are never overwritten automatically."}
          </span>
        </div>
        <div className="flex items-center gap-2">
          {onChangeUploadMode && (
            <label
              className="flex items-center gap-1.5 text-[11px] text-commander-subtext cursor-pointer"
              title="When enabled, local saves are automatically uploaded to the remote server after verifying no remote changes occurred"
            >
              <input
                type="checkbox"
                checked={uploadMode === "auto"}
                onChange={(e) => void onChangeUploadMode(e.target.checked ? "auto" : "prompt")}
                className="rounded border-commander-border bg-commander-dark text-commander-accent"
              />
              <span>Auto-upload on save</span>
            </label>
          )}
          {onScanNow && (
            <button
              type="button"
              onClick={() => void onScanNow()}
              className="flex items-center gap-1 px-2 py-0.5 rounded bg-commander-dark hover:bg-commander-surface border border-commander-border text-commander-subtext hover:text-white text-[11px] transition-colors"
              title="Check temporary working copies for local modifications now"
            >
              <RefreshCw className="w-3 h-3" />
              <span>Check Changes</span>
            </button>
          )}
        </div>
      </div>

      {notice && (
        <div
          role={notice.kind === "error" ? "alert" : "status"}
          className={`flex items-center justify-between gap-2 px-2.5 py-1.5 rounded border ${
            notice.kind === "error"
              ? "bg-rose-950/80 border-rose-800 text-rose-200"
              : "bg-emerald-950/70 border-emerald-800 text-emerald-200"
          }`}
        >
          <div className="flex items-center gap-2 min-w-0">
            {notice.kind === "error" ? (
              <AlertCircle className="w-3.5 h-3.5 text-rose-400 shrink-0" />
            ) : (
              <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
            )}
            <span className="truncate">{notice.message}</span>
          </div>
          <button
            type="button"
            onClick={() => setNotice(null)}
            className="p-0.5 rounded hover:bg-black/30 text-current"
            title="Dismiss notice"
          >
            <X className="w-3 h-3" />
          </button>
        </div>
      )}

      <div className="space-y-1.5 max-h-48 overflow-y-auto">
        {sessions.map((session) => {
          const isBusy = busySessionId === session.id || session.state.state === "Uploading";
          const isConfirmingDiscard = confirmDiscardId === session.id;
          const isDirty = session.has_unsynced_changes || session.state.state !== "Clean";
          const endpointLabel = `${session.endpoint.username}@${session.endpoint.host}:${session.endpoint.port}${
            session.endpoint.elevated ? " (sudo)" : ""
          }`;

          return (
            <div
              key={session.id}
              className={`rounded border p-2 flex flex-col gap-1.5 ${
                session.state.state === "Conflict"
                  ? "bg-amber-950/60 border-amber-700/80 text-amber-100"
                  : session.state.state === "UploadFailed" || session.state.state === "LocalMissing"
                    ? "bg-rose-950/60 border-rose-800/80 text-rose-100"
                    : session.state.state === "Modified"
                      ? "bg-sky-950/50 border-sky-700/70 text-sky-100"
                      : "bg-commander-dark/80 border-commander-border text-commander-text"
              }`}
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-2 min-w-0">
                  {session.state.state === "Conflict" ? (
                    <AlertTriangle className="w-4 h-4 text-amber-400 shrink-0" />
                  ) : session.state.state === "UploadFailed" ||
                    session.state.state === "LocalMissing" ? (
                    <AlertCircle className="w-4 h-4 text-rose-400 shrink-0" />
                  ) : session.state.state === "Modified" ? (
                    <Upload className="w-4 h-4 text-sky-400 shrink-0" />
                  ) : session.state.state === "Uploading" ? (
                    <Loader2 className="w-4 h-4 text-commander-accent animate-spin shrink-0" />
                  ) : (
                    <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
                  )}

                  <div className="min-w-0">
                    <div className="flex items-center gap-1.5 flex-wrap">
                      <span className="font-semibold truncate">{session.file_name}</span>
                      <span className="text-[10px] font-mono px-1.5 py-0.2 rounded bg-black/30 border border-white/10">
                        {session.state.state === "Clean"
                          ? "Synced"
                          : session.state.state === "Modified"
                            ? session.prompt_dismissed
                              ? "Modified (Kept Local)"
                              : "Modified Locally"
                            : session.state.state === "Uploading"
                              ? "Uploading..."
                              : session.state.state === "Conflict"
                                ? "Remote Conflict"
                                : session.state.state === "UploadFailed"
                                  ? "Upload Failed"
                                  : "Temp File Missing"}
                      </span>
                      {session.app_name && (
                        <span className="text-[10px] text-commander-subtext">
                          via {session.app_name}
                        </span>
                      )}
                      {session.recovered && (
                        <span className="text-[10px] px-1.5 py-0.2 rounded bg-amber-900/60 border border-amber-700 text-amber-200">
                          Recovered Session
                        </span>
                      )}
                    </div>
                    <div className="text-[11px] font-mono opacity-80 truncate">
                      {endpointLabel}:{session.remote_path} → {session.local_path}
                    </div>
                  </div>
                </div>

                {/* Action Controls */}
                <div className="flex items-center flex-wrap gap-1.5 shrink-0">
                  {session.state.state === "Modified" && (
                    <>
                      <button
                        type="button"
                        disabled={isBusy}
                        onClick={() =>
                          void runAction(session.id, () => onUploadSession(session.id, false))
                        }
                        className="flex items-center gap-1 px-2.5 py-1 rounded bg-commander-accent hover:bg-commander-accent/90 text-commander-dark font-semibold transition-colors disabled:opacity-50"
                      >
                        <Upload className="w-3 h-3" />
                        <span>Upload Changes</span>
                      </button>
                      {!session.prompt_dismissed && (
                        <button
                          type="button"
                          disabled={isBusy}
                          onClick={() =>
                            void runAction(session.id, () => onKeepLocalChanges(session.id))
                          }
                          className="px-2 py-1 rounded bg-commander-dark hover:bg-commander-surface border border-commander-border text-commander-text transition-colors disabled:opacity-50"
                        >
                          Keep Local Changes
                        </button>
                      )}
                      {onReloadFromRemote && isConnected && (
                        <button
                          type="button"
                          disabled={isBusy}
                          onClick={() =>
                            void runAction(session.id, () => onReloadFromRemote(session.id))
                          }
                          className="flex items-center gap-1 px-2 py-1 rounded bg-commander-dark hover:bg-commander-surface border border-commander-border text-commander-text transition-colors disabled:opacity-50"
                          title="Discard local changes, reload the latest remote file, and reopen it in the editor"
                        >
                          <RefreshCw className="w-3 h-3" />
                          <span>Load Remote Version</span>
                        </button>
                      )}
                    </>
                  )}

                  {session.state.state === "UploadFailed" && (
                    <>
                      <button
                        type="button"
                        disabled={isBusy}
                        onClick={() =>
                          void runAction(session.id, () => onUploadSession(session.id, false))
                        }
                        className="flex items-center gap-1 px-2.5 py-1 rounded bg-commander-accent hover:bg-commander-accent/90 text-commander-dark font-semibold transition-colors disabled:opacity-50"
                      >
                        <RefreshCw className="w-3 h-3" />
                        <span>Retry Upload</span>
                      </button>
                      <button
                        type="button"
                        disabled={isBusy}
                        onClick={() =>
                          void runAction(session.id, async () => {
                            const saved = await onSaveLocalCopy(session.id, localCurrentDir);
                            if (saved) {
                              setNotice({
                                kind: "info",
                                message: `Saved local edited copy to ${saved}`,
                              });
                            }
                          })
                        }
                        className="flex items-center gap-1 px-2 py-1 rounded bg-commander-dark hover:bg-commander-surface border border-commander-border text-commander-text transition-colors disabled:opacity-50"
                      >
                        <Copy className="w-3 h-3" />
                        <span>Save Local Copy</span>
                      </button>
                    </>
                  )}

                  {session.state.state === "Conflict" && (
                    <>
                      <button
                        type="button"
                        disabled={isBusy}
                        onClick={() =>
                          void runAction(session.id, () => onUploadSession(session.id, true))
                        }
                        className="flex items-center gap-1 px-2.5 py-1 rounded bg-amber-500 hover:bg-amber-400 text-commander-dark font-semibold transition-colors disabled:opacity-50"
                        title="Explicitly overwrite the changed remote file with your local working copy"
                      >
                        <Upload className="w-3 h-3" />
                        <span>Overwrite Remote</span>
                      </button>
                      <button
                        type="button"
                        disabled={isBusy}
                        onClick={() =>
                          void runAction(session.id, async () => {
                            const saved = await onSaveLocalCopy(session.id, localCurrentDir);
                            if (saved) {
                              setNotice({
                                kind: "info",
                                message: `Saved local copy to ${saved} for manual comparison`,
                              });
                            }
                          })
                        }
                        className="flex items-center gap-1 px-2 py-1 rounded bg-commander-dark hover:bg-commander-surface border border-commander-border text-commander-text transition-colors disabled:opacity-50"
                      >
                        <Copy className="w-3 h-3" />
                        <span>Save Copy Under New Name</span>
                      </button>
                      {onReloadFromRemote && isConnected && (
                        <button
                          type="button"
                          disabled={isBusy}
                          onClick={() =>
                            void runAction(session.id, () => onReloadFromRemote(session.id))
                          }
                          className="flex items-center gap-1 px-2 py-1 rounded bg-commander-dark hover:bg-commander-surface border border-commander-border text-commander-text transition-colors disabled:opacity-50"
                          title="Replace the local working copy with the newer remote file and reopen it in the editor"
                        >
                          <RefreshCw className="w-3 h-3" />
                          <span>Load Remote Version</span>
                        </button>
                      )}
                      {onDownloadNewerRemote && isConnected && (
                        <button
                          type="button"
                          disabled={isBusy}
                          onClick={() =>
                            void runAction(session.id, () =>
                              onDownloadNewerRemote(session.remote_path, localCurrentDir)
                            )
                          }
                          className="px-2 py-1 rounded bg-commander-dark hover:bg-commander-surface border border-commander-border text-commander-text transition-colors disabled:opacity-50"
                        >
                          Download Newer Remote
                        </button>
                      )}
                      {!session.prompt_dismissed && (
                        <button
                          type="button"
                          disabled={isBusy}
                          onClick={() =>
                            void runAction(session.id, () => onKeepLocalChanges(session.id))
                          }
                          className="px-2 py-1 rounded bg-commander-dark hover:bg-commander-surface border border-commander-border text-commander-text transition-colors disabled:opacity-50"
                        >
                          Keep Remote &amp; Hold Local
                        </button>
                      )}
                    </>
                  )}

                  {onChooseAppForRemote && (
                    <button
                      type="button"
                      disabled={isBusy}
                      onClick={() => onChooseAppForRemote(session.remote_path, session.file_name)}
                      className="flex items-center gap-1 px-2 py-1 rounded bg-commander-dark hover:bg-commander-surface border border-commander-border text-commander-subtext hover:text-white transition-colors"
                      title="Open with another local application"
                    >
                      <ExternalLink className="w-3 h-3" />
                      <span>Open With...</span>
                    </button>
                  )}

                  {!isConfirmingDiscard ? (
                    <button
                      type="button"
                      disabled={isBusy}
                      onClick={() => {
                        if (isDirty) {
                          setConfirmDiscardId(session.id);
                        } else {
                          void runAction(session.id, () => onDiscardSession(session.id, false));
                        }
                      }}
                      className="flex items-center gap-1 px-2 py-1 rounded bg-commander-dark hover:bg-rose-950/70 border border-commander-border text-commander-subtext hover:text-rose-200 transition-colors disabled:opacity-50"
                      title={
                        isDirty
                          ? "Discard local temporary edit"
                          : "Close remote editing session and remove clean temporary file"
                      }
                    >
                      <Trash2 className="w-3 h-3" />
                      <span>{isDirty ? "Discard Edit..." : "Close & Clean Up"}</span>
                    </button>
                  ) : (
                    <div className="flex items-center gap-1 bg-rose-950/90 border border-rose-700 rounded px-2 py-0.5">
                      <span className="text-[11px] text-rose-200">Discard unsaved local edit?</span>
                      <button
                        type="button"
                        disabled={isBusy}
                        onClick={() => {
                          setConfirmDiscardId(null);
                          void runAction(session.id, () => onDiscardSession(session.id, true));
                        }}
                        className="px-2 py-0.5 rounded bg-rose-700 hover:bg-rose-600 text-white font-semibold text-[11px]"
                      >
                        Confirm Discard
                      </button>
                      <button
                        type="button"
                        onClick={() => setConfirmDiscardId(null)}
                        className="px-1.5 py-0.5 rounded bg-commander-dark text-commander-subtext hover:text-white text-[11px]"
                      >
                        Cancel
                      </button>
                    </div>
                  )}
                </div>
              </div>

              {/* Conflict / Error Diagnostic Details */}
              {session.state.state === "Conflict" && (
                <div role="alert" className="text-[11px] text-amber-200/95 pl-6">
                  <strong>Conflict prevented automatic overwrite:</strong>{" "}
                  {session.state.details.reason}
                  {session.state.details.remote_current?.size !== undefined &&
                    session.state.details.remote_current?.size !== null && (
                      <span className="ml-2 font-mono">
                        (Baseline: {formatFileSize(session.baseline.size ?? 0, false)} → Remote now:{" "}
                        {formatFileSize(session.state.details.remote_current.size, false)})
                      </span>
                    )}
                </div>
              )}

              {session.state.state === "UploadFailed" && (
                <div role="alert" className="text-[11px] text-rose-200/95 pl-6">
                  <strong>Upload failed ({session.state.details.kind}):</strong>{" "}
                  {session.state.details.message}
                </div>
              )}

              {session.state.state === "LocalMissing" && (
                <div role="alert" className="text-[11px] text-rose-200/95 pl-6">
                  The local temporary working copy at <code>{session.local_path}</code> was removed
                  or moved outside LinuxSCP.
                </div>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
};
