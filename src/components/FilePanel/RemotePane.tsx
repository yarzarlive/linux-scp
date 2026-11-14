import React, { useState, useEffect, useCallback, useMemo, useRef } from "react";
import {
  Server,
  Unplug,
  Shield,
  ShieldAlert,
  RefreshCw,
  Loader2,
  AlertTriangle,
  AlertCircle,
  CheckCircle2,
  WifiOff,
  ArrowLeft,
  ArrowUp,
  Download,
  FolderPlus,
  FilePlus,
  Edit3,
  Trash2,
  SlidersHorizontal,
} from "lucide-react";
import { AddressBar } from "./AddressBar";
import { ColumnHeader, SortColumn } from "./ColumnHeader";
import { FileTable } from "./FileTable";
import { ContextMenu, ContextMenuPosition } from "./ContextMenu";
import { InputDialog } from "../Common/InputDialog";
import { ConfirmDialog } from "../Common/ConfirmDialog";
import { PropertiesDialog } from "../PropertiesDialog/PropertiesDialog";
import { OpenWithDialog } from "../OpenWithDialog/OpenWithDialog";
import { PaneDropIndicator } from "./PaneDropIndicator";
import { FileEntry } from "../../types/filesystem";
import { RemoteEditSession } from "../../types/desktop";
import { SessionStateSnapshot } from "../../types/session";
import { AppError } from "../../types/ipc";
import { IpcClient } from "../../services/ipc";
import { formatFileSize, isDirectory, isSymlink } from "../../utils/formatters";
import { InternalDragPayload } from "../../utils/dragDrop";
import { usePaneDragDrop } from "../../hooks/usePaneDragDrop";
import {
  RemoteNavHistoryState,
  createInitialRemoteNavState,
  pushRemoteHistory,
  stepRemoteHistoryBack,
  stepRemoteHistoryForward,
} from "../../utils/remoteNavigation";

interface RemotePaneProps {
  sessionState?: SessionStateSnapshot;
  connectionError?: string | null;
  onConnect?: () => void;
  onDisconnect?: () => void;
  onReconnect?: () => void;
  onDirectoryChange?: (path: string) => void;
  onDownload?: (paths: string[], targetLocalDir?: string) => void;
  onDropUpload?: (localPaths: string[], targetRemoteDir: string) => void;
  activeDrag?: InternalDragPayload | null;
  onInternalDragStart?: (payload: InternalDragPayload) => void;
  onInternalDragEnd?: () => void;
  isActivePane?: boolean;
  onFocusPane?: () => void;
  refreshTrigger?: number;
  onRemoteEditOpened?: (session: RemoteEditSession) => void;
}

type RemoteErrorKind = "PermissionDenied" | "Network" | "Other";

interface RemoteDirectoryError {
  kind: RemoteErrorKind;
  message: string;
  targetPath?: string;
}

const DEFAULT_SESSION_STATE: SessionStateSnapshot = {
  status: { status: "Disconnected" },
  session: null,
  trusted_host_key: null,
};

function classifyRemoteError(err: unknown, targetPath?: string): RemoteDirectoryError {
  if (typeof err === "object" && err !== null && "kind" in err && "message" in err) {
    const appErr = err as AppError;
    if (appErr.kind === "PermissionDenied") {
      return { kind: "PermissionDenied", message: appErr.message, targetPath };
    }
    if (appErr.kind === "Network") {
      return { kind: "Network", message: appErr.message, targetPath };
    }
    return { kind: "Other", message: appErr.message, targetPath };
  }
  const msg = err instanceof Error ? err.message : String(err ?? "Could not read remote directory");
  return { kind: "Other", message: msg, targetPath };
}

export const RemotePane: React.FC<RemotePaneProps> = ({
  sessionState = DEFAULT_SESSION_STATE,
  connectionError = null,
  onConnect,
  onDisconnect,
  onReconnect,
  onDirectoryChange,
  onDownload,
  onDropUpload,
  activeDrag = null,
  onInternalDragStart,
  onInternalDragEnd,
  isActivePane = false,
  onFocusPane,
  refreshTrigger = 0,
  onRemoteEditOpened,
}) => {
  const statusKind = sessionState.status.status;
  const activeSession = statusKind === "Connected" ? sessionState.session : null;
  const trustedHostKey = sessionState.trusted_host_key;

  const isConnecting = statusKind === "Connecting" || statusKind === "Reconnecting";
  const isFailed = statusKind === "Failed";
  const isConnected = statusKind === "Connected" && Boolean(activeSession);

  const connectionKey = activeSession
    ? (activeSession.profile_id ?? activeSession.session_id)
    : null;

  // Per-connection navigation history, sort, and hidden-file preferences
  const [navByConnection, setNavByConnection] = useState<Record<string, RemoteNavHistoryState>>({});

  const [currentPath, setCurrentPath] = useState<string>("");
  const [parentPath, setParentPath] = useState<string | null>(null);
  const [homePath, setHomePath] = useState<string | null>(null);
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [selectedPaths, setSelectedPaths] = useState<Set<string>>(new Set());
  const [focusedPath, setFocusedPath] = useState<string | undefined>();
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [remoteError, setRemoteError] = useState<RemoteDirectoryError | null>(null);

  // Remote file operation dialogs
  const [createDirOpen, setCreateDirOpen] = useState(false);
  const [createFileOpen, setCreateFileOpen] = useState(false);
  const [renameOpen, setRenameOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [propertiesOpen, setPropertiesOpen] = useState(false);
  const [recursiveDeleteTargets, setRecursiveDeleteTargets] = useState<FileEntry[]>([]);
  const [dialogError, setDialogError] = useState<string | null>(null);
  const [isOperating, setIsOperating] = useState(false);
  const [contextMenuPos, setContextMenuPos] = useState<ContextMenuPosition | null>(null);
  const [openWithTarget, setOpenWithTarget] = useState<{
    path: string;
    name: string;
    missingAppId?: string | null;
  } | null>(null);
  const [directDownloadPath, setDirectDownloadPath] = useState<string>("");

  const loadedSessionIdRef = useRef<string | null>(null);
  const requestSeqRef = useRef<number>(0);

  const currentNav: RemoteNavHistoryState = useMemo(() => {
    if (!connectionKey) return createInitialRemoteNavState();
    return navByConnection[connectionKey] ?? createInitialRemoteNavState();
  }, [connectionKey, navByConnection]);

  const updateCurrentNav = useCallback(
    (updater: (prev: RemoteNavHistoryState) => RemoteNavHistoryState) => {
      if (!connectionKey) return;
      setNavByConnection((prevMap) => {
        const existing = prevMap[connectionKey] ?? createInitialRemoteNavState();
        return {
          ...prevMap,
          [connectionKey]: updater(existing),
        };
      });
    },
    [connectionKey]
  );

  const loadDirectory = useCallback(
    async (
      targetPath?: string,
      recordHistory = true,
      overrideShowHidden?: boolean
    ): Promise<void> => {
      if (!isConnected || !activeSession) return;
      if (!activeSession.capabilities.can_list_directory) return;

      const seq = ++requestSeqRef.current;
      const effectiveHidden = overrideShowHidden ?? currentNav.showHidden;

      setIsLoading(true);
      setRemoteError(null);

      try {
        const listing = await IpcClient.listRemoteDirectory(targetPath, effectiveHidden);
        if (seq !== requestSeqRef.current) return;

        setCurrentPath(listing.current_path);
        setParentPath(listing.parent_path ?? null);
        if (listing.home_path) {
          setHomePath(listing.home_path);
        }
        setEntries(listing.entries);
        setSelectedPaths(new Set());
        setFocusedPath(undefined);
        onDirectoryChange?.(listing.current_path);

        if (recordHistory) {
          updateCurrentNav((prev) => pushRemoteHistory(prev, listing.current_path));
        }
      } catch (err) {
        if (seq !== requestSeqRef.current) return;
        setRemoteError(classifyRemoteError(err, targetPath));
      } finally {
        if (seq === requestSeqRef.current) {
          setIsLoading(false);
        }
      }
    },
    [isConnected, activeSession, currentNav.showHidden, updateCurrentNav, onDirectoryChange]
  );

  // Automatically load initial/restored directory when a session becomes Connected
  useEffect(() => {
    if (isConnected && activeSession) {
      if (loadedSessionIdRef.current !== activeSession.session_id) {
        loadedSessionIdRef.current = activeSession.session_id;
        setCurrentPath(activeSession.current_remote_path);
        setHomePath(activeSession.home_remote_path ?? null);
        setEntries([]);
        setSelectedPaths(new Set());
        setFocusedPath(undefined);
        setRemoteError(null);
        if (activeSession.capabilities.can_list_directory) {
          loadDirectory(activeSession.current_remote_path, true);
        }
      }
    } else {
      loadedSessionIdRef.current = null;
      requestSeqRef.current += 1;
      setEntries([]);
      setSelectedPaths(new Set());
      setFocusedPath(undefined);
      setRemoteError(null);
      setIsLoading(false);
      if (statusKind === "Disconnected") {
        setCurrentPath("");
        setParentPath(null);
        setHomePath(null);
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isConnected, activeSession?.session_id, statusKind]);

  // Reload remote directory when uploads complete
  useEffect(() => {
    if (refreshTrigger > 0 && isConnected && activeSession) {
      if (activeSession.capabilities.can_list_directory) {
        const target = currentPath || activeSession.current_remote_path;
        if (target) {
          loadDirectory(target, false);
        }
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshTrigger]);

  // Navigation handlers
  const handleNavigate = (targetPath: string) => {
    if (activeSession && !activeSession.capabilities.can_list_directory) {
      setCurrentPath(targetPath);
      onDirectoryChange?.(targetPath);
      return;
    }
    loadDirectory(targetPath, true);
  };

  const handleBack = () => {
    if (!connectionKey) return;
    const { state: nextState, targetPath } = stepRemoteHistoryBack(currentNav);
    if (targetPath) {
      updateCurrentNav(() => nextState);
      loadDirectory(targetPath, false);
    }
  };

  const handleForward = () => {
    if (!connectionKey) return;
    const { state: nextState, targetPath } = stepRemoteHistoryForward(currentNav);
    if (targetPath) {
      updateCurrentNav(() => nextState);
      loadDirectory(targetPath, false);
    }
  };

  const handleUp = () => {
    if (parentPath) {
      loadDirectory(parentPath, true);
    }
  };

  const handleHome = () => {
    const target = homePath || activeSession?.home_remote_path || "~";
    loadDirectory(target, true);
  };

  const handleRoot = () => {
    loadDirectory("/", true);
  };

  const handleRefresh = () => {
    if (currentPath) {
      loadDirectory(currentPath, false);
    } else if (activeSession) {
      loadDirectory(activeSession.current_remote_path, false);
    }
  };

  const handleToggleHidden = () => {
    const nextHidden = !currentNav.showHidden;
    updateCurrentNav((prev) => ({ ...prev, showHidden: nextHidden }));
    if (currentPath) {
      loadDirectory(currentPath, false, nextHidden);
    }
  };

  const handleSortChange = (col: SortColumn) => {
    updateCurrentNav((prev) => {
      if (prev.sortBy === col) {
        return { ...prev, sortDirection: prev.sortDirection === "asc" ? "desc" : "asc" };
      }
      return { ...prev, sortBy: col, sortDirection: "asc" };
    });
  };

  const handleOpenEntry = async (entry: FileEntry) => {
    if (isDirectory(entry.file_type)) {
      loadDirectory(entry.path, true);
      return;
    }

    if (isSymlink(entry.file_type)) {
      try {
        const targetIsDir = await IpcClient.checkRemotePathIsDir(entry.path);
        if (targetIsDir) {
          loadDirectory(entry.path, true);
          return;
        }
      } catch (err) {
        setRemoteError(classifyRemoteError(err, entry.path));
        return;
      }
    }

    setRemoteError(null);
    try {
      const outcome = await IpcClient.openRemoteFileDefault(entry.path);
      if (outcome.outcome === "NeedsAppPicker") {
        setOpenWithTarget({
          path: entry.path,
          name: entry.name,
          missingAppId: outcome.missing_remembered_app_id ?? null,
        });
      } else if (outcome.outcome === "Launched") {
        onRemoteEditOpened?.(outcome.session);
      }
    } catch (err) {
      setRemoteError(classifyRemoteError(err, entry.path));
    }
  };

  const handleOpenWithEntry = (entry: FileEntry) => {
    if (isDirectory(entry.file_type)) return;
    setRemoteError(null);
    setOpenWithTarget({
      path: entry.path,
      name: entry.name,
      missingAppId: null,
    });
  };

  const summary = useMemo(() => {
    const totalCount = entries.length;
    let selectedCount = 0;
    let selectedBytes = 0;

    for (const entry of entries) {
      if (selectedPaths.has(entry.path)) {
        selectedCount++;
        if (!isDirectory(entry.file_type) && entry.metadata.size) {
          selectedBytes += entry.metadata.size;
        }
      }
    }

    return { totalCount, selectedCount, selectedBytes };
  }, [entries, selectedPaths]);

  const errorMessage =
    connectionError ||
    (sessionState.status.status === "Failed" ? sessionState.status.details.error_message : null);

  const canGoBack = currentNav.historyIndex > 0;
  const canGoForward = currentNav.historyIndex < currentNav.history.length - 1;
  const canGoUp = Boolean(parentPath && parentPath !== currentPath);
  const displayedPath =
    isConnected && activeSession
      ? currentPath || activeSession.current_remote_path
      : "sftp://[not connected]/";

  const handleSelectionChange = useCallback((paths: Set<string>, focus?: string) => {
    setSelectedPaths(paths);
    setFocusedPath(focus);
  }, []);

  const selectedEntries = useMemo(
    () => entries.filter((e) => selectedPaths.has(e.path)),
    [entries, selectedPaths]
  );

  const singleSelectedEntry = selectedEntries.length === 1 ? selectedEntries[0] : null;

  // Capability-aware permissions for remote operations
  const caps = activeSession?.capabilities;
  const canCreateDir = Boolean(isConnected && caps?.can_create_directory);
  const canCreateFile = Boolean(isConnected && caps?.can_create_file);
  const canRename = Boolean(isConnected && caps?.can_rename);
  const canDeleteFiles = Boolean(isConnected && caps?.can_delete_file);
  const canDeleteDirs = Boolean(isConnected && caps?.can_delete_directory);
  const canSetPermissions = Boolean(isConnected && caps?.can_set_permissions);
  const canSetOwnership = Boolean(isConnected && caps?.can_set_owner_group);
  const canOpenProperties = Boolean(
    isConnected && selectedEntries.length > 0 && (canSetPermissions || canSetOwnership)
  );
  const canDeleteSelected =
    selectedEntries.length > 0 &&
    selectedEntries.every((e) => (isDirectory(e.file_type) ? canDeleteDirs : canDeleteFiles));

  const extractRemoteOpError = (err: unknown, fallback: string) => {
    if (err && typeof err === "object" && "message" in err) {
      return String((err as { message: unknown }).message);
    }
    if (err instanceof Error) return err.message;
    return fallback;
  };

  const handleOpenCreateDir = () => {
    if (!canCreateDir) return;
    setDialogError(null);
    setCreateDirOpen(true);
  };

  const handleOpenCreateFile = () => {
    if (!canCreateFile) return;
    setDialogError(null);
    setCreateFileOpen(true);
  };

  const handleOpenRename = () => {
    if (!canRename || selectedPaths.size !== 1) return;
    setDialogError(null);
    setRenameOpen(true);
  };

  const handleOpenDelete = () => {
    if (!canDeleteSelected) return;
    setDialogError(null);
    setDeleteOpen(true);
  };

  const handleConfirmCreateDir = async (dirName: string) => {
    setIsOperating(true);
    setDialogError(null);
    try {
      await IpcClient.createRemoteDirectory(displayedPath, dirName);
      setCreateDirOpen(false);
      await loadDirectory(displayedPath, false);
    } catch (err: unknown) {
      setDialogError(extractRemoteOpError(err, "Failed to create remote directory"));
    } finally {
      setIsOperating(false);
    }
  };

  const handleConfirmCreateFile = async (fileName: string) => {
    setIsOperating(true);
    setDialogError(null);
    try {
      await IpcClient.createRemoteFile(displayedPath, fileName);
      setCreateFileOpen(false);
      await loadDirectory(displayedPath, false);
    } catch (err: unknown) {
      setDialogError(extractRemoteOpError(err, "Failed to create remote file"));
    } finally {
      setIsOperating(false);
    }
  };

  const handleConfirmRename = async (newName: string) => {
    if (!singleSelectedEntry) return;
    setIsOperating(true);
    setDialogError(null);
    try {
      await IpcClient.renameRemoteEntry(singleSelectedEntry.path, newName);
      setRenameOpen(false);
      await loadDirectory(displayedPath, false);
    } catch (err: unknown) {
      setDialogError(extractRemoteOpError(err, "Failed to rename remote item"));
    } finally {
      setIsOperating(false);
    }
  };

  const handleConfirmDelete = async () => {
    if (!canDeleteSelected) return;
    setIsOperating(true);
    setDialogError(null);

    let anyDeleted = false;
    const nonEmptyDirs: FileEntry[] = [];

    try {
      for (const entry of selectedEntries) {
        if (isDirectory(entry.file_type)) {
          try {
            await IpcClient.deleteRemoteDirectory(entry.path, false);
            anyDeleted = true;
          } catch (err: unknown) {
            const msg = extractRemoteOpError(err, "Failed to delete remote directory");
            if (msg.toLowerCase().includes("not empty")) {
              nonEmptyDirs.push(entry);
            } else {
              throw err;
            }
          }
        } else {
          await IpcClient.deleteRemoteFile(entry.path);
          anyDeleted = true;
        }
      }

      setDeleteOpen(false);
      if (anyDeleted) {
        await loadDirectory(displayedPath, false);
      }
      if (nonEmptyDirs.length > 0) {
        setDialogError(null);
        setRecursiveDeleteTargets(nonEmptyDirs);
      }
    } catch (err: unknown) {
      if (anyDeleted) {
        await loadDirectory(displayedPath, false);
      }
      setDialogError(extractRemoteOpError(err, "Failed to delete remote items"));
    } finally {
      setIsOperating(false);
    }
  };

  const handleConfirmRecursiveDelete = async () => {
    if (!canDeleteDirs || recursiveDeleteTargets.length === 0) return;
    setIsOperating(true);
    setDialogError(null);
    let anyDeleted = false;

    try {
      for (const entry of recursiveDeleteTargets) {
        await IpcClient.deleteRemoteDirectory(entry.path, true);
        anyDeleted = true;
      }
      setRecursiveDeleteTargets([]);
      await loadDirectory(displayedPath, false);
    } catch (err: unknown) {
      if (anyDeleted) {
        await loadDirectory(displayedPath, false);
      }
      setDialogError(extractRemoteOpError(err, "Failed to recursively delete remote directory"));
    } finally {
      setIsOperating(false);
    }
  };

  const handleContextMenu = (e: React.MouseEvent, entry?: FileEntry) => {
    if (!isConnected) return;
    if (entry && !selectedPaths.has(entry.path)) {
      setSelectedPaths(new Set([entry.path]));
      setFocusedPath(entry.path);
    }
    setContextMenuPos({ x: e.clientX, y: e.clientY });
  };

  const {
    dropState,
    handleEntryDragStart,
    handleEntryDragEnd,
    handlePaneDragOver,
    handleRowDragOver,
    handlePaneDragLeave,
    handleRowDragLeave,
    handlePaneDrop,
    handleRowDrop,
  } = usePaneDragDrop({
    pane: "remote",
    currentPath:
      isConnected && activeSession ? currentPath || activeSession.current_remote_path : "",
    isReady: isConnected && Boolean(activeSession) && !isLoading && !remoteError,
    entries,
    selectedPaths,
    onSelectionChange: handleSelectionChange,
    activeDrag,
    onInternalDragStart,
    onInternalDragEnd,
    onAcceptDrop: onDropUpload,
  });

  return (
    <section
      role="region"
      aria-label="Remote Filesystem"
      onClick={onFocusPane}
      onDragOver={handlePaneDragOver}
      onDragLeave={handlePaneDragLeave}
      onDrop={handlePaneDrop}
      data-drop-status={dropState.status}
      className={`flex flex-col h-full rounded border overflow-hidden bg-commander-surface transition-colors ${
        dropState.status === "valid"
          ? "border-emerald-500 ring-1 ring-emerald-500/60"
          : dropState.status === "invalid"
            ? "border-rose-500/70"
            : isActivePane
              ? "border-commander-accent/50"
              : "border-commander-border"
      }`}
    >
      {/* Pane Title Bar */}
      <div className="flex items-center justify-between px-3 py-1.5 bg-commander-darker border-b border-commander-border text-xs font-medium select-none gap-2">
        <div className="flex items-center gap-1.5 text-commander-subtext min-w-0">
          <Server className="w-3.5 h-3.5 shrink-0" />
          <span className="shrink-0">Remote Filesystem</span>
          {activeSession && (
            <span className="ml-1 font-mono text-[11px] text-commander-text truncate">
              ({activeSession.username}@{activeSession.host}:{activeSession.port})
            </span>
          )}
          {activeSession?.protocol === "SCP" && (
            <span
              className="ml-1 px-2 py-0.5 rounded bg-sky-500/20 text-sky-300 border border-sky-500/50 text-[10px] font-bold tracking-wider uppercase shrink-0 flex items-center gap-1 shadow-sm"
              title="Connected via SCP protocol (Transfer-Only mode)"
            >
              <Shield className="w-3 h-3 text-sky-400 shrink-0" />
              <span>SCP (TRANSFER-ONLY)</span>
            </span>
          )}
        </div>
        <div className="flex items-center gap-1 shrink-0">
          {isConnected && activeSession ? (
            <>
              {canCreateDir && (
                <button
                  type="button"
                  onClick={handleOpenCreateDir}
                  className="p-1 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-text hover:text-white transition-colors"
                  title="New Remote Folder (F7)"
                  aria-label="New Remote Folder (F7)"
                >
                  <FolderPlus className="w-3.5 h-3.5 text-amber-400" />
                </button>
              )}
              {canCreateFile && (
                <button
                  type="button"
                  onClick={handleOpenCreateFile}
                  className="p-1 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-text hover:text-white transition-colors"
                  title="New Remote File"
                  aria-label="New Remote File"
                >
                  <FilePlus className="w-3.5 h-3.5 text-commander-accent" />
                </button>
              )}
              {canRename && (
                <button
                  type="button"
                  onClick={handleOpenRename}
                  disabled={selectedPaths.size !== 1}
                  className={`p-1 rounded border transition-colors ${
                    selectedPaths.size === 1
                      ? "bg-commander-dark hover:bg-commander-selected border-commander-border text-commander-text hover:text-white cursor-pointer"
                      : "bg-commander-dark/40 border-commander-border/40 text-commander-subtext/30 cursor-not-allowed"
                  }`}
                  title="Rename Selected Remote Item (F2)"
                  aria-label="Rename Selected Remote Item (F2)"
                >
                  <Edit3 className="w-3.5 h-3.5 text-sky-400" />
                </button>
              )}
              {(canDeleteFiles || canDeleteDirs) && (
                <button
                  type="button"
                  onClick={handleOpenDelete}
                  disabled={!canDeleteSelected}
                  className={`p-1 rounded border transition-colors ${
                    canDeleteSelected
                      ? "bg-rose-950/40 hover:bg-rose-900/60 border-rose-900/60 text-rose-300 cursor-pointer"
                      : "bg-commander-dark/40 border-commander-border/40 text-commander-subtext/30 cursor-not-allowed"
                  }`}
                  title="Delete Selected Remote Items (Del)"
                  aria-label="Delete Selected Remote Items (Del)"
                >
                  <Trash2 className="w-3.5 h-3.5 text-rose-400" />
                </button>
              )}
              {(canSetPermissions || canSetOwnership) && (
                <button
                  type="button"
                  onClick={() => setPropertiesOpen(true)}
                  disabled={!canOpenProperties}
                  className={`p-1 rounded border transition-colors ${
                    canOpenProperties
                      ? "bg-commander-dark hover:bg-commander-selected border-commander-border text-commander-text hover:text-white cursor-pointer"
                      : "bg-commander-dark/40 border-commander-border/40 text-commander-subtext/30 cursor-not-allowed"
                  }`}
                  title="Remote File Properties & Permissions"
                  aria-label="Remote File Properties & Permissions"
                >
                  <SlidersHorizontal className="w-3.5 h-3.5 text-amber-400" />
                </button>
              )}
              {onDownload && caps?.can_download && (
                <button
                  type="button"
                  onClick={() => onDownload(Array.from(selectedPaths))}
                  disabled={selectedPaths.size === 0}
                  className={`p-1 rounded border transition-colors ${
                    selectedPaths.size > 0
                      ? "bg-emerald-950/60 hover:bg-emerald-900/70 border-emerald-700/60 text-emerald-200 cursor-pointer"
                      : "bg-commander-dark/40 border-commander-border/40 text-commander-subtext/30 cursor-not-allowed"
                  }`}
                  title="Download Selected to Local Pane"
                  aria-label="Download Selected to Local Pane"
                >
                  <Download className="w-3.5 h-3.5 text-emerald-400" />
                </button>
              )}
              <span className="flex items-center gap-1 text-[10px] text-emerald-400 bg-emerald-950/40 px-1.5 py-0.5 rounded border border-emerald-800/50">
                <CheckCircle2 className="w-3 h-3" />
                <span>Connected</span>
              </span>
              {onReconnect && (
                <button
                  type="button"
                  onClick={onReconnect}
                  className="p-1 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-text hover:text-white transition-colors"
                  title="Reconnect SFTP Session"
                  aria-label="Reconnect SFTP Session"
                >
                  <RefreshCw className="w-3.5 h-3.5 text-commander-accent" />
                </button>
              )}
              {onDisconnect && (
                <button
                  type="button"
                  onClick={onDisconnect}
                  className="p-1 rounded bg-rose-950/40 hover:bg-rose-950/70 border border-rose-800/60 text-rose-300 transition-colors"
                  title="Disconnect SFTP Session"
                  aria-label="Disconnect SFTP Session"
                >
                  <Unplug className="w-3.5 h-3.5" />
                </button>
              )}
            </>
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
        path={displayedPath}
        paneType="remote"
        siteId={
          activeSession
            ? (activeSession.profile_id ??
              `${activeSession.username}@${activeSession.host}:${activeSession.port}`)
            : undefined
        }
        onNavigate={handleNavigate}
        onBack={handleBack}
        canGoBack={
          isConnected && Boolean(activeSession?.capabilities.can_list_directory) && canGoBack
        }
        onForward={handleForward}
        canGoForward={
          isConnected && Boolean(activeSession?.capabilities.can_list_directory) && canGoForward
        }
        onUp={handleUp}
        canGoUp={isConnected && Boolean(activeSession?.capabilities.can_list_directory) && canGoUp}
        onHome={activeSession?.capabilities.can_list_directory ? handleHome : undefined}
        onRoot={activeSession?.capabilities.can_list_directory ? handleRoot : undefined}
        onRefresh={activeSession?.capabilities.can_list_directory ? handleRefresh : undefined}
        showHidden={currentNav.showHidden}
        onToggleHidden={
          activeSession?.capabilities.can_list_directory ? handleToggleHidden : undefined
        }
        disabled={!isConnected}
        isLoading={isLoading}
      />

      {/* Main Content Area by Lifecycle & Browsing State */}
      {isConnecting ? (
        <>
          <ColumnHeader disabled={true} />
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
        </>
      ) : isConnected && activeSession ? (
        !activeSession.capabilities.can_list_directory ? (
          <div
            className="flex-1 flex flex-col p-4 bg-commander-surface/30 overflow-y-auto select-none"
            data-testid="scp-transfer-panel"
          >
            {/* SCP Capability Banner */}
            <div className="p-3.5 mb-4 rounded-md border border-sky-800/60 bg-sky-950/40 text-xs">
              <div className="flex items-center gap-2 text-sky-300 font-semibold mb-1">
                <Shield className="w-4 h-4 text-sky-400" />
                <span>SCP Protocol Active • Transfer-Only Mode</span>
              </div>
              <p className="text-commander-subtext text-[11px] leading-relaxed mb-2">
                Legacy SCP operates as a streaming transfer mechanism over an SSH exec channel. It
                does not support remote directory browsing, file listing, permissions management, or
                interactive file editing.
              </p>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-2 text-[11px] pt-2 border-t border-sky-900/50">
                <div>
                  <span className="text-emerald-400 font-medium">Supported Operations:</span>
                  <ul className="list-disc list-inside text-commander-subtext pl-1 mt-0.5 space-y-0.5">
                    <li>Upload files to remote target path</li>
                    <li>Direct download of remote file by path</li>
                    <li>Transfer queue progress, cancel, and speed</li>
                    <li>Strict SSH host-key verification and Keyring</li>
                  </ul>
                </div>
                <div>
                  <span className="text-amber-400 font-medium">Explicitly Unsupported:</span>
                  <ul className="list-disc list-inside text-commander-subtext pl-1 mt-0.5 space-y-0.5">
                    <li>Remote directory browsing and file listing</li>
                    <li>Remote file creation, rename, and deletion</li>
                    <li>Chmod and chown ownership modification</li>
                    <li>Sudo-SFTP elevation and remote file editing</li>
                  </ul>
                </div>
              </div>
            </div>

            {/* Target Upload Path & Drag-and-Drop Zone */}
            <div className="p-4 mb-4 rounded-md border border-commander-border bg-commander-darker/60 flex flex-col gap-3">
              <div className="flex items-center justify-between">
                <span className="text-xs font-medium text-commander-text flex items-center gap-1.5">
                  <Server className="w-3.5 h-3.5 text-commander-accent" />
                  <span>Target Remote Upload Directory</span>
                </span>
                <span className="text-[11px] font-mono text-commander-text bg-commander-surface px-2 py-0.5 rounded border border-commander-border">
                  {displayedPath}
                </span>
              </div>

              <div
                className={`border-2 border-dashed rounded-lg p-6 text-center transition-colors ${
                  dropState.status === "valid"
                    ? "border-emerald-500 bg-emerald-950/20 text-emerald-300"
                    : dropState.status === "invalid"
                      ? "border-rose-500 bg-rose-950/20 text-rose-300"
                      : "border-commander-border/80 hover:border-commander-accent/60 bg-commander-dark/30 text-commander-subtext"
                }`}
              >
                <Download className="w-8 h-8 mx-auto mb-2 text-commander-accent/70 rotate-180" />
                <div className="text-xs font-medium text-commander-text mb-1">
                  Drop files from Local pane here to upload via SCP
                </div>
                <p className="text-[11px] text-commander-subtext">
                  Transfers will be queued and sent safely to{" "}
                  <span className="font-mono text-commander-text">{displayedPath}</span>
                </p>
              </div>
            </div>

            {/* Direct Remote File Download Card */}
            <div className="p-4 rounded-md border border-commander-border bg-commander-darker/60 flex flex-col gap-2">
              <div className="flex items-center gap-1.5 text-xs font-medium text-commander-text">
                <Download className="w-3.5 h-3.5 text-emerald-400" />
                <span>Direct Remote File Download</span>
              </div>
              <p className="text-[11px] text-commander-subtext">
                Enter an absolute or home-relative remote file path to download to your local
                working directory:
              </p>
              <form
                onSubmit={(e) => {
                  e.preventDefault();
                  if (directDownloadPath.trim() && onDownload) {
                    onDownload([directDownloadPath.trim()]);
                    setDirectDownloadPath("");
                  }
                }}
                className="flex items-center gap-2 mt-1"
              >
                <input
                  type="text"
                  value={directDownloadPath}
                  onChange={(e) => setDirectDownloadPath(e.target.value)}
                  placeholder="/remote/path/to/file.ext"
                  className="flex-1 px-2.5 py-1.5 rounded bg-commander-dark border border-commander-border text-xs text-commander-text font-mono placeholder:text-commander-subtext/40 focus:outline-none focus:border-commander-accent"
                />
                <button
                  type="submit"
                  disabled={!directDownloadPath.trim()}
                  className={`px-3 py-1.5 rounded text-xs font-medium flex items-center gap-1 transition-colors ${
                    directDownloadPath.trim()
                      ? "bg-emerald-950/60 hover:bg-emerald-900/70 text-emerald-200 border border-emerald-700/60 cursor-pointer"
                      : "bg-commander-dark/40 border border-commander-border/40 text-commander-subtext/30 cursor-not-allowed"
                  }`}
                >
                  <Download className="w-3.5 h-3.5 text-emerald-400" />
                  <span>Download</span>
                </button>
              </form>
            </div>
          </div>
        ) : remoteError?.kind === "PermissionDenied" ? (
          <>
            <ColumnHeader
              sortBy={currentNav.sortBy}
              sortDirection={currentNav.sortDirection}
              onSortChange={handleSortChange}
              disabled={true}
            />
            <div
              role="alert"
              className="flex-1 flex flex-col items-center justify-center p-6 text-center text-commander-subtext select-none bg-commander-surface/40"
            >
              <div className="w-14 h-14 rounded-full bg-amber-950/40 border border-amber-800/60 flex items-center justify-center mb-3">
                <ShieldAlert className="w-7 h-7 text-amber-400" />
              </div>
              <h3 className="text-sm font-medium text-amber-200 mb-1">Permission Denied</h3>
              <p className="text-xs text-amber-300/90 max-w-md mb-4 p-2.5 rounded bg-amber-950/30 border border-amber-900/50 font-mono break-words select-text">
                {remoteError.message}
              </p>
              <div className="flex items-center gap-2">
                {canGoBack && (
                  <button
                    type="button"
                    onClick={handleBack}
                    className="flex items-center gap-1.5 px-3 py-1.5 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-text text-xs font-medium transition-colors"
                  >
                    <ArrowLeft className="w-3.5 h-3.5" />
                    <span>Go Back</span>
                  </button>
                )}
                {canGoUp && (
                  <button
                    type="button"
                    onClick={handleUp}
                    className="flex items-center gap-1.5 px-3 py-1.5 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-text text-xs font-medium transition-colors"
                  >
                    <ArrowUp className="w-3.5 h-3.5" />
                    <span>Go to Parent</span>
                  </button>
                )}
                <button
                  type="button"
                  onClick={handleRefresh}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-text text-xs font-medium transition-colors"
                >
                  <RefreshCw className="w-3.5 h-3.5 text-commander-accent" />
                  <span>Retry</span>
                </button>
              </div>
            </div>
          </>
        ) : remoteError?.kind === "Network" ? (
          <>
            <ColumnHeader
              sortBy={currentNav.sortBy}
              sortDirection={currentNav.sortDirection}
              onSortChange={handleSortChange}
              disabled={true}
            />
            <div
              role="alert"
              className="flex-1 flex flex-col items-center justify-center p-6 text-center text-commander-subtext select-none bg-commander-surface/40"
            >
              <div className="w-14 h-14 rounded-full bg-rose-950/40 border border-rose-800/60 flex items-center justify-center mb-3">
                <WifiOff className="w-7 h-7 text-rose-400" />
              </div>
              <h3 className="text-sm font-medium text-rose-200 mb-1">Remote Network Error</h3>
              <p className="text-xs text-rose-300/90 max-w-md mb-4 p-2.5 rounded bg-rose-950/30 border border-rose-900/50 font-mono break-words select-text">
                {remoteError.message}
              </p>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={handleRefresh}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-text text-xs font-medium transition-colors"
                >
                  <RefreshCw className="w-3.5 h-3.5 text-commander-accent" />
                  <span>Retry</span>
                </button>
                {onReconnect && (
                  <button
                    type="button"
                    onClick={onReconnect}
                    className="flex items-center gap-1.5 px-3 py-1.5 rounded bg-commander-accent/20 hover:bg-commander-accent/30 text-commander-accent border border-commander-accent/40 text-xs font-medium transition-colors"
                  >
                    <RefreshCw className="w-3.5 h-3.5" />
                    <span>Reconnect</span>
                  </button>
                )}
              </div>
            </div>
          </>
        ) : (
          <>
            {remoteError && (
              <div
                role="alert"
                className="flex items-center justify-between p-2 bg-rose-950/80 border-b border-rose-800 text-rose-200 text-xs"
              >
                <div className="flex items-center gap-1.5 min-w-0">
                  <AlertCircle className="w-4 h-4 shrink-0 text-rose-400" />
                  <span className="truncate">{remoteError.message}</span>
                </div>
                <div className="flex items-center gap-2 shrink-0 ml-2">
                  {canGoUp && (
                    <button
                      type="button"
                      onClick={handleUp}
                      className="px-2 py-0.5 rounded bg-rose-900 hover:bg-rose-800 text-white font-medium"
                    >
                      Go Up
                    </button>
                  )}
                  <button
                    type="button"
                    onClick={handleRefresh}
                    className="px-2 py-0.5 rounded bg-rose-900 hover:bg-rose-800 text-white font-medium"
                  >
                    Retry
                  </button>
                </div>
              </div>
            )}

            <FileTable
              entries={entries}
              selectedPaths={selectedPaths}
              onSelectionChange={handleSelectionChange}
              focusedPath={focusedPath}
              onOpenEntry={handleOpenEntry}
              onNavigateParent={canGoUp ? handleUp : undefined}
              sortBy={currentNav.sortBy}
              sortDirection={currentNav.sortDirection}
              onSortChange={handleSortChange}
              isLoading={isLoading}
              onContextMenu={handleContextMenu}
              onDeleteSelected={canDeleteSelected ? handleOpenDelete : undefined}
              onRenameSelected={
                canRename && selectedPaths.size === 1 ? handleOpenRename : undefined
              }
              onNewFolder={canCreateDir ? handleOpenCreateDir : undefined}
              onEntryDragStart={handleEntryDragStart}
              onEntryDragEnd={handleEntryDragEnd}
              onRowDragOver={handleRowDragOver}
              onRowDragLeave={handleRowDragLeave}
              onRowDrop={handleRowDrop}
              dropHighlightPath={dropState.status !== "idle" ? dropState.hoveredEntryPath : null}
              dropHighlightKind={dropState.status !== "idle" ? dropState.status : null}
            />
          </>
        )
      ) : isFailed ? (
        <>
          <ColumnHeader disabled={true} />
          <div className="flex-1 flex flex-col items-center justify-center p-6 text-center text-commander-subtext select-none bg-commander-surface/40">
            <div className="w-14 h-14 rounded-full bg-rose-950/40 border border-rose-800/60 flex items-center justify-center mb-3">
              <AlertTriangle className="w-7 h-7 text-rose-400" />
            </div>
            <h3 className="text-sm font-medium text-rose-200 mb-1">
              Remote SFTP Connection Failed
            </h3>
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
        </>
      ) : (
        <>
          <ColumnHeader disabled={true} />
          <div className="flex-1 flex flex-col items-center justify-center p-6 text-center text-commander-subtext select-none bg-commander-surface/40">
            <div className="w-16 h-16 rounded-full bg-commander-dark border border-commander-border flex items-center justify-center mb-3">
              <Server className="w-8 h-8 opacity-40 text-commander-accent" />
            </div>
            <h3 className="text-sm font-medium text-commander-text mb-1">
              No Remote Connection Established
            </h3>
            <p className="text-xs text-commander-subtext/70 max-w-sm mb-4">
              Connect to an SSH/SFTP server using strict host-key verification and secure Linux
              Secret Service credentials to browse remote files.
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
        </>
      )}

      {/* Internal Drop Target Indicator (Bottom of Pane) */}
      <PaneDropIndicator dropState={dropState} itemCount={activeDrag?.sourcePaths.length ?? 0} />

      {/* Status Bar */}
      <div className="flex items-center justify-between px-3 py-1 bg-commander-darker border-t border-commander-border text-[11px] text-commander-subtext select-none">
        <span>
          {isConnected && activeSession ? (
            !activeSession.capabilities.can_list_directory ? (
              <span className="text-sky-300 font-medium">
                SCP Mode • Direct transfer queue only (Browsing & file management unsupported)
              </span>
            ) : (
              <>
                <span>{summary.totalCount} items</span>
                {summary.selectedCount > 0 && (
                  <span className="ml-2 text-commander-accent">
                    • {summary.selectedCount} selected (
                    {formatFileSize(summary.selectedBytes, false)})
                  </span>
                )}
              </>
            )
          ) : isConnecting ? (
            "Handshaking SSH & SFTP..."
          ) : isFailed ? (
            "Connection Failed"
          ) : (
            "Disconnected"
          )}
        </span>
        <span className="text-[10px] opacity-70">
          {trustedHostKey
            ? `Host Key Verified (${trustedHostKey.algorithm})`
            : "Strict Host-Key Enforced"}
        </span>
      </div>

      {/* Capability-Aware Remote Context Menu */}
      <ContextMenu
        position={contextMenuPos}
        onClose={() => setContextMenuPos(null)}
        hasSelection={selectedPaths.size > 0}
        isSingleSelection={selectedPaths.size === 1}
        isDirectorySelection={!!singleSelectedEntry && isDirectory(singleSelectedEntry.file_type)}
        onOpen={singleSelectedEntry ? () => void handleOpenEntry(singleSelectedEntry) : undefined}
        onOpenWith={
          singleSelectedEntry && !isDirectory(singleSelectedEntry.file_type)
            ? () => handleOpenWithEntry(singleSelectedEntry)
            : undefined
        }
        onNewFolder={canCreateDir ? handleOpenCreateDir : undefined}
        onNewFile={canCreateFile ? handleOpenCreateFile : undefined}
        onRename={canRename && singleSelectedEntry ? handleOpenRename : undefined}
        onDelete={canDeleteSelected ? handleOpenDelete : undefined}
        onProperties={canOpenProperties ? () => setPropertiesOpen(true) : undefined}
        onRefresh={handleRefresh}
      />

      {/* Remote Open With Application Picker Dialog */}
      <OpenWithDialog
        isOpen={Boolean(openWithTarget)}
        mode="remote"
        filePath={openWithTarget?.path ?? null}
        fileName={openWithTarget?.name ?? null}
        missingRememberedAppId={openWithTarget?.missingAppId}
        onRemoteLaunched={(result) => onRemoteEditOpened?.(result.session)}
        onClose={() => setOpenWithTarget(null)}
      />

      {/* Remote Properties / Permissions / Ownership Dialog */}
      <PropertiesDialog
        isOpen={propertiesOpen}
        entries={selectedEntries}
        canSetPermissions={canSetPermissions}
        canSetOwnership={canSetOwnership}
        onResolveIdentity={(uid, gid) => IpcClient.resolveRemoteIdentity(uid, gid)}
        onApplyPermissions={async (paths, octalMode, recursive) => {
          await IpcClient.setRemotePermissions(paths, octalMode, recursive);
          await loadDirectory(displayedPath, false);
        }}
        onApplyOwnership={async (path, uid, gid, userName, groupName) => {
          await IpcClient.setRemoteOwnership(path, uid, gid, userName, groupName);
          await loadDirectory(displayedPath, false);
        }}
        onClose={() => setPropertiesOpen(false)}
      />

      {/* Create Remote Directory Dialog */}
      <InputDialog
        isOpen={createDirOpen}
        title="Create New Remote Folder"
        description={`Enter folder name to create in: ${displayedPath}`}
        placeholder="Folder name"
        confirmLabel="Create Folder"
        isLoading={isOperating}
        error={dialogError}
        onConfirm={handleConfirmCreateDir}
        onCancel={() => setCreateDirOpen(false)}
      />

      {/* Create Remote File Dialog */}
      <InputDialog
        isOpen={createFileOpen}
        title="Create New Remote File"
        description={`Enter file name to create in: ${displayedPath}`}
        placeholder="example.txt"
        confirmLabel="Create File"
        isLoading={isOperating}
        error={dialogError}
        onConfirm={handleConfirmCreateFile}
        onCancel={() => setCreateFileOpen(false)}
      />

      {/* Rename Remote Item Dialog */}
      <InputDialog
        isOpen={renameOpen}
        title="Rename Remote Item"
        description={`Enter new name for: ${singleSelectedEntry?.name || ""}`}
        initialValue={singleSelectedEntry?.name || ""}
        placeholder="New name"
        confirmLabel="Rename"
        isLoading={isOperating}
        error={dialogError}
        onConfirm={handleConfirmRename}
        onCancel={() => setRenameOpen(false)}
      />

      {/* Standard Remote Delete Confirmation Dialog (Non-Recursive for Directories) */}
      <ConfirmDialog
        isOpen={deleteOpen}
        title="Confirm Remote Deletion"
        description={
          activeSession?.elevation.type === "ElevatedSudo"
            ? "ELEVATED (root) session: Are you sure you want to delete the selected remote item(s)? Directories are deleted only if empty."
            : "Are you sure you want to delete the selected remote item(s)? Directories are deleted only if empty."
        }
        items={selectedEntries.map((e) => e.path)}
        confirmLabel="Delete"
        isDestructive={true}
        isLoading={isOperating}
        error={dialogError}
        onConfirm={handleConfirmDelete}
        onCancel={() => setDeleteOpen(false)}
      />

      {/* Explicit Recursive Directory Deletion Confirmation Dialog */}
      <ConfirmDialog
        isOpen={recursiveDeleteTargets.length > 0}
        title="Confirm Recursive Directory Deletion"
        description={
          activeSession?.elevation.type === "ElevatedSudo"
            ? "WARNING (ELEVATED ROOT SESSION): The following remote directory/directories are NOT empty. Confirming will permanently delete ALL nested files and subdirectories inside them."
            : "WARNING: The following remote directory/directories are NOT empty. Confirming will permanently delete ALL nested files and subdirectories inside them."
        }
        items={recursiveDeleteTargets.map((e) => e.path)}
        confirmLabel="Delete Recursively"
        isDestructive={true}
        isLoading={isOperating}
        error={dialogError}
        onConfirm={handleConfirmRecursiveDelete}
        onCancel={() => {
          setRecursiveDeleteTargets([]);
          setDialogError(null);
        }}
      />
    </section>
  );
};
