import React, { useState, useEffect, useMemo, useCallback } from "react";
import {
  ExternalLink,
  Search,
  AlertCircle,
  AlertTriangle,
  CheckCircle2,
  Terminal,
  Trash2,
  X,
  RefreshCw,
} from "lucide-react";
import { IpcClient } from "../../services/ipc";
import { DesktopAppInfo, LocalFileLaunchResult, OpenWithContext } from "../../types/desktop";

interface OpenWithDialogProps {
  isOpen: boolean;
  filePath: string | null;
  fileName: string | null;
  missingRememberedAppId?: string | null;
  onClose: () => void;
  onLaunched?: (result: LocalFileLaunchResult) => void;
}

export const OpenWithDialog: React.FC<OpenWithDialogProps> = ({
  isOpen,
  filePath,
  fileName,
  missingRememberedAppId = null,
  onClose,
  onLaunched,
}) => {
  const [context, setContext] = useState<OpenWithContext | null>(null);
  const [selectedAppId, setSelectedAppId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [rememberDefault, setRememberDefault] = useState<boolean>(false);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [isLaunching, setIsLaunching] = useState<boolean>(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const loadApplications = useCallback(async () => {
    if (!filePath) return;
    setIsLoading(true);
    setErrorMessage(null);
    try {
      const ctx = await IpcClient.discoverDesktopApplications(filePath);
      setContext(ctx);

      if (ctx.remembered_app_id && ctx.remembered_app_installed) {
        setSelectedAppId(ctx.remembered_app_id);
        setRememberDefault(false);
      } else if (ctx.recommended_app_ids.length > 0) {
        setSelectedAppId(ctx.recommended_app_ids[0]);
        setRememberDefault(Boolean(missingRememberedAppId));
      } else if (ctx.applications.length > 0) {
        setSelectedAppId(ctx.applications[0].id);
        setRememberDefault(Boolean(missingRememberedAppId));
      } else {
        setSelectedAppId(null);
      }
    } catch (err: unknown) {
      const msg =
        err && typeof err === "object" && "message" in err
          ? String((err as { message: unknown }).message)
          : "Failed to discover installed desktop applications";
      setErrorMessage(msg);
    } finally {
      setIsLoading(false);
    }
  }, [filePath, missingRememberedAppId]);

  useEffect(() => {
    if (isOpen && filePath) {
      setSearchQuery("");
      loadApplications();
    }
  }, [isOpen, filePath, loadApplications]);

  const sortedAndFilteredApps = useMemo(() => {
    if (!context) return [];
    const recommendedSet = new Set(context.recommended_app_ids);
    const defaultId = context.remembered_app_id;

    const q = searchQuery.trim().toLowerCase();
    const filtered = context.applications.filter((app) => {
      if (!q) return true;
      return (
        app.name.toLowerCase().includes(q) ||
        app.id.toLowerCase().includes(q) ||
        app.executable.toLowerCase().includes(q) ||
        (app.comment && app.comment.toLowerCase().includes(q))
      );
    });

    return [...filtered].sort((a, b) => {
      const aIsDefault = a.id === defaultId ? 1 : 0;
      const bIsDefault = b.id === defaultId ? 1 : 0;
      if (aIsDefault !== bIsDefault) return bIsDefault - aIsDefault;

      const aIsRec = recommendedSet.has(a.id) ? 1 : 0;
      const bIsRec = recommendedSet.has(b.id) ? 1 : 0;
      if (aIsRec !== bIsRec) return bIsRec - aIsRec;

      return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
    });
  }, [context, searchQuery]);

  if (!isOpen || !filePath) return null;

  const rememberedApp: DesktopAppInfo | undefined = context?.applications.find(
    (a) => a.id === context.remembered_app_id
  );
  const uninstalledAppId =
    missingRememberedAppId ||
    (context?.remembered_app_id && !context.remembered_app_installed
      ? context.remembered_app_id
      : null);

  const handleLaunch = async (overrideAppId?: string) => {
    const targetAppId = overrideAppId || selectedAppId;
    if (!targetAppId) {
      setErrorMessage("Please select an application from the list.");
      return;
    }

    setIsLaunching(true);
    setErrorMessage(null);
    try {
      const result = await IpcClient.openLocalFileWithApp(filePath, targetAppId, rememberDefault);
      onLaunched?.(result);
      onClose();
    } catch (err: unknown) {
      const msg =
        err && typeof err === "object" && "message" in err
          ? String((err as { message: unknown }).message)
          : "Failed to launch selected application";
      setErrorMessage(msg);
    } finally {
      setIsLaunching(false);
    }
  };

  const handleClearAssociation = async () => {
    if (!context) return;
    setErrorMessage(null);
    try {
      await IpcClient.deleteFileAppAssociation(context.extension_key);
      await loadApplications();
    } catch (err: unknown) {
      const msg =
        err && typeof err === "object" && "message" in err
          ? String((err as { message: unknown }).message)
          : "Failed to clear default association";
      setErrorMessage(msg);
    }
  };

  const recommendedSet = new Set(context?.recommended_app_ids ?? []);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4 select-none"
      role="dialog"
      aria-modal="true"
      aria-labelledby="open-with-dialog-title"
    >
      <div className="w-full max-w-xl rounded-lg border border-commander-border bg-commander-surface shadow-2xl flex flex-col overflow-hidden">
        {/* Title Bar */}
        <div className="flex items-center justify-between px-4 py-2.5 bg-commander-darker border-b border-commander-border">
          <div className="flex items-center gap-2">
            <ExternalLink className="w-4 h-4 text-commander-accent" />
            <h2 id="open-with-dialog-title" className="text-sm font-semibold text-commander-text">
              Open With — Choose Application
            </h2>
          </div>
          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={loadApplications}
              disabled={isLoading}
              className="p-1 rounded hover:bg-commander-dark text-commander-subtext hover:text-white transition-colors"
              title="Refresh installed applications"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isLoading ? "animate-spin" : ""}`} />
            </button>
            <button
              type="button"
              onClick={onClose}
              className="p-1 rounded hover:bg-commander-dark text-commander-subtext hover:text-white transition-colors"
              title="Close dialog"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Body */}
        <div className="p-4 space-y-3 text-xs">
          {/* File Summary */}
          <div className="rounded border border-commander-border bg-commander-darker/60 p-2.5 flex items-center justify-between gap-2">
            <div className="min-w-0">
              <div className="font-semibold text-commander-text truncate">
                {fileName || filePath.split("/").pop()}
              </div>
              <div className="text-[11px] font-mono text-commander-subtext truncate">
                {filePath}
              </div>
            </div>
            {context && (
              <span className="shrink-0 text-[10px] font-mono px-2 py-0.5 rounded bg-commander-dark border border-commander-border text-commander-accent">
                {context.extension_display}
              </span>
            )}
          </div>

          {/* Uninstalled Default Warning */}
          {uninstalledAppId && (
            <div
              role="alert"
              className="flex items-center justify-between gap-2 p-2.5 rounded bg-amber-950/80 border border-amber-800 text-amber-200"
            >
              <div className="flex items-center gap-2">
                <AlertTriangle className="w-4 h-4 shrink-0 text-amber-400" />
                <span>
                  The saved default application (<code>{uninstalledAppId}</code>) is no longer
                  installed. Choose a new application below.
                </span>
              </div>
              <button
                type="button"
                onClick={handleClearAssociation}
                className="px-2 py-0.5 rounded bg-amber-900/60 hover:bg-amber-900 text-amber-100 text-[11px] shrink-0"
              >
                Clear Stale Default
              </button>
            </div>
          )}

          {/* Current Remembered Default Info */}
          {rememberedApp && (
            <div className="flex items-center justify-between gap-2 p-2 rounded bg-emerald-950/40 border border-emerald-800/60 text-emerald-200">
              <div className="flex items-center gap-2 min-w-0">
                <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                <span className="truncate">
                  LinuxSCP default for <strong>{context?.extension_display}</strong>:{" "}
                  <strong>{rememberedApp.name}</strong>
                </span>
              </div>
              <button
                type="button"
                onClick={handleClearAssociation}
                className="flex items-center gap-1 px-2 py-0.5 rounded bg-commander-dark hover:bg-rose-950/60 border border-commander-border text-commander-subtext hover:text-rose-200 text-[11px] shrink-0 transition-colors"
                title="Remove remembered default application for this extension"
              >
                <Trash2 className="w-3 h-3" />
                <span>Reset Default</span>
              </button>
            </div>
          )}

          {/* Error Banner */}
          {errorMessage && (
            <div
              role="alert"
              className="flex items-center gap-2 p-2.5 rounded bg-rose-950/80 border border-rose-800 text-rose-200"
            >
              <AlertCircle className="w-4 h-4 shrink-0 text-rose-400" />
              <span>{errorMessage}</span>
            </div>
          )}

          {/* Search Input */}
          <div className="relative">
            <Search className="w-3.5 h-3.5 absolute left-2.5 top-2 text-commander-subtext/60" />
            <input
              type="text"
              aria-label="Filter applications"
              placeholder="Search installed applications by name, description, or binary..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full pl-8 pr-3 py-1.5 text-xs bg-commander-dark border border-commander-border rounded text-commander-text placeholder-commander-subtext/40 focus:outline-none focus:border-commander-accent"
            />
          </div>

          {/* Applications List */}
          <div
            role="listbox"
            aria-label="Installed Desktop Applications"
            className="h-64 overflow-y-auto rounded border border-commander-border bg-commander-darker/60 divide-y divide-commander-border/40"
          >
            {isLoading ? (
              <div className="h-full flex items-center justify-center text-commander-subtext">
                Discovering installed XDG desktop applications...
              </div>
            ) : sortedAndFilteredApps.length === 0 ? (
              <div className="h-full flex items-center justify-center text-commander-subtext p-4 text-center">
                {context?.applications.length === 0
                  ? "No XDG desktop applications were found in standard application directories."
                  : "No installed applications match your filter."}
              </div>
            ) : (
              sortedAndFilteredApps.map((app) => {
                const isSelected = selectedAppId === app.id;
                const isCurrentDefault = app.id === context?.remembered_app_id;
                const isRecommended = recommendedSet.has(app.id);

                return (
                  <div
                    key={app.id}
                    role="option"
                    aria-selected={isSelected}
                    onClick={() => setSelectedAppId(app.id)}
                    onDoubleClick={() => handleLaunch(app.id)}
                    className={`flex items-center justify-between p-2.5 cursor-pointer transition-colors ${
                      isSelected
                        ? "bg-commander-selected text-white"
                        : "hover:bg-commander-dark/80 text-commander-text"
                    }`}
                  >
                    <div className="min-w-0 flex-1 pr-2">
                      <div className="flex items-center gap-1.5">
                        <span className="font-medium truncate">{app.name}</span>
                        {isCurrentDefault && (
                          <span className="text-[10px] px-1.5 py-0.2 rounded bg-emerald-950/80 border border-emerald-700/70 text-emerald-300">
                            Default
                          </span>
                        )}
                        {!isCurrentDefault && isRecommended && (
                          <span className="text-[10px] px-1.5 py-0.2 rounded bg-sky-950/80 border border-sky-700/70 text-sky-300">
                            Recommended
                          </span>
                        )}
                        {app.terminal && (
                          <span
                            className="flex items-center gap-0.5 text-[10px] px-1.5 py-0.2 rounded bg-commander-dark border border-commander-border text-commander-subtext"
                            title="Runs in terminal"
                          >
                            <Terminal className="w-2.5 h-2.5" />
                            <span>Terminal</span>
                          </span>
                        )}
                      </div>
                      {app.comment && (
                        <div className="text-[11px] text-commander-subtext truncate mt-0.5">
                          {app.comment}
                        </div>
                      )}
                      <div className="text-[10px] font-mono text-commander-subtext/70 truncate mt-0.5">
                        {app.id} ({app.executable})
                      </div>
                    </div>
                  </div>
                );
              })
            )}
          </div>

          {/* Remember Default Toggle */}
          <label className="flex items-start gap-2 pt-1 cursor-pointer">
            <input
              type="checkbox"
              checked={rememberDefault}
              onChange={(e) => setRememberDefault(e.target.checked)}
              className="mt-0.5 rounded border-commander-border bg-commander-dark text-commander-accent"
            />
            <div>
              <div className="text-xs text-commander-text font-medium">
                Remember as LinuxSCP default application for{" "}
                <code>{context?.extension_display ?? "this file type"}</code>
              </div>
              <div className="text-[11px] text-commander-subtext">
                Stored only inside LinuxSCP&apos;s database; system-wide MIME defaults are not
                modified.
              </div>
            </div>
          </label>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-4 py-3 bg-commander-darker border-t border-commander-border">
          <button
            type="button"
            onClick={onClose}
            disabled={isLaunching}
            className="px-3 py-1.5 text-xs rounded border border-commander-border hover:bg-commander-dark text-commander-subtext hover:text-white transition-colors"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => handleLaunch()}
            disabled={!selectedAppId || isLaunching || isLoading}
            className="px-4 py-1.5 text-xs rounded font-medium bg-commander-accent hover:bg-commander-accent/90 text-commander-dark transition-colors disabled:opacity-50 cursor-pointer"
          >
            {isLaunching ? "Opening..." : "Open File"}
          </button>
        </div>
      </div>
    </div>
  );
};

