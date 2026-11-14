import React, { useState, useEffect, useCallback, useMemo } from "react";
import { Folder, AlertCircle, FolderPlus, FilePlus, Edit3, Trash2, Upload } from "lucide-react";
import { FileEntry } from "../../types/filesystem";
import { IpcClient } from "../../services/ipc";
import { AddressBar } from "./AddressBar";
import { FileTable } from "./FileTable";
import { SortColumn, SortDirection } from "./ColumnHeader";
import { ContextMenu, ContextMenuPosition } from "./ContextMenu";
import { InputDialog } from "../Common/InputDialog";
import { ConfirmDialog } from "../Common/ConfirmDialog";
import { OpenWithDialog } from "../OpenWithDialog/OpenWithDialog";
import { PaneDropIndicator } from "./PaneDropIndicator";
import { formatFileSize, isDirectory } from "../../utils/formatters";
import { InternalDragPayload } from "../../utils/dragDrop";
import { usePaneDragDrop } from "../../hooks/usePaneDragDrop";

interface LocalPaneProps {
  initialPath?: string;
  onDirectoryChange?: (path: string) => void;
  canUpload?: boolean;
  onUpload?: (paths: string[], targetRemoteDir?: string) => void;
  onDropDownload?: (remotePaths: string[], targetLocalDir: string) => void;
  activeDrag?: InternalDragPayload | null;
  onInternalDragStart?: (payload: InternalDragPayload) => void;
  onInternalDragEnd?: () => void;
  isActivePane?: boolean;
  onFocusPane?: () => void;
  refreshTrigger?: number;
}

export const LocalPane: React.FC<LocalPaneProps> = ({
  initialPath,
  onDirectoryChange,
  canUpload = false,
  onUpload,
  onDropDownload,
  activeDrag = null,
  onInternalDragStart,
  onInternalDragEnd,
  isActivePane = true,
  onFocusPane,
  refreshTrigger = 0,
}) => {
  const [currentPath, setCurrentPath] = useState<string>(initialPath || "");
  const [parentPath, setParentPath] = useState<string | null>(null);
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState<number>(-1);
  const [showHidden, setShowHidden] = useState<boolean>(false);
  const [sortBy, setSortBy] = useState<SortColumn>("name");
  const [sortDirection, setSortDirection] = useState<SortDirection>("asc");
  const [selectedPaths, setSelectedPaths] = useState<Set<string>>(new Set());
  const [focusedPath, setFocusedPath] = useState<string | undefined>();
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  // Dialog states
  const [createDirOpen, setCreateDirOpen] = useState(false);
  const [createFileOpen, setCreateFileOpen] = useState(false);
  const [renameOpen, setRenameOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [dialogError, setDialogError] = useState<string | null>(null);
  const [isOperating, setIsOperating] = useState(false);
  const [openWithTarget, setOpenWithTarget] = useState<{
    path: string;
    name: string;
    missingAppId?: string | null;
  } | null>(null);

  // Context menu state
  const [contextMenuPos, setContextMenuPos] = useState<ContextMenuPosition | null>(null);

  // Load a directory by path
  const loadDirectory = useCallback(
    async (targetPath?: string, recordHistory = true) => {
      setIsLoading(true);
      setError(null);

      try {
        const listing = await IpcClient.listLocalDirectory(targetPath, showHidden);
        setCurrentPath(listing.current_path);
        setParentPath(listing.parent_path ?? null);
        setEntries(listing.entries);
        setSelectedPaths(new Set());
        setFocusedPath(undefined);
        onDirectoryChange?.(listing.current_path);

        if (recordHistory) {
          setHistory((prev) => {
            const next = prev.slice(0, historyIndex + 1);
            return [...next, listing.current_path];
          });
          setHistoryIndex((prev) => prev + 1);
        }
      } catch (err: unknown) {
        console.error("Failed to load local directory:", err);
        const msg =
          err && typeof err === "object" && "message" in err
            ? String(err.message)
            : "Could not read directory";
        setError(msg);
      } finally {
        setIsLoading(false);
      }
    },
    [showHidden, historyIndex, onDirectoryChange]
  );

  // Initialize with home directory or initial path
  useEffect(() => {
    async function init() {
      if (initialPath) {
        await loadDirectory(initialPath, true);
      } else {
        try {
          const home = await IpcClient.getHomeDirectory();
          await loadDirectory(home, true);
        } catch {
          await loadDirectory("/", true);
        }
      }
    }
    init();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Reload when showHidden changes
  useEffect(() => {
    if (currentPath) {
      loadDirectory(currentPath, false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showHidden]);

  // Reload when external transfers complete
  useEffect(() => {
    if (refreshTrigger > 0 && currentPath) {
      loadDirectory(currentPath, false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshTrigger]);

  // Navigation handlers
  const handleNavigate = (path: string) => {
    loadDirectory(path, true);
  };

  const handleBack = () => {
    if (historyIndex > 0) {
      const prevPath = history[historyIndex - 1];
      setHistoryIndex((idx) => idx - 1);
      loadDirectory(prevPath, false);
    }
  };

  const handleForward = () => {
    if (historyIndex < history.length - 1) {
      const nextPath = history[historyIndex + 1];
      setHistoryIndex((idx) => idx + 1);
      loadDirectory(nextPath, false);
    }
  };

  const handleUp = () => {
    if (parentPath) {
      loadDirectory(parentPath, true);
    }
  };

  const handleHome = async () => {
    try {
      const home = await IpcClient.getHomeDirectory();
      loadDirectory(home, true);
    } catch {
      loadDirectory("/", true);
    }
  };

  const handleRoot = () => {
    loadDirectory("/", true);
  };

  const handleRefresh = () => {
    if (currentPath) {
      loadDirectory(currentPath, false);
    }
  };

  const handleOpenEntry = async (entry: FileEntry) => {
    if (isDirectory(entry.file_type)) {
      loadDirectory(entry.path, true);
      return;
    }

    setError(null);
    try {
      const outcome = await IpcClient.openLocalFileDefault(entry.path);
      if (outcome.outcome === "NeedsAppPicker") {
        setOpenWithTarget({
          path: entry.path,
          name: entry.name,
          missingAppId: outcome.missing_remembered_app_id ?? null,
        });
      }
    } catch (err: unknown) {
      const msg =
        err && typeof err === "object" && "message" in err
          ? String(err.message)
          : "Failed to open local file";
      if (entry.file_type.type === "Symlink" && msg.includes("is a directory")) {
        await loadDirectory(entry.path, true);
        return;
      }
      setError(msg);
    }
  };

  const handleOpenWithEntry = (entry: FileEntry) => {
    if (isDirectory(entry.file_type)) return;
    setError(null);
    setOpenWithTarget({
      path: entry.path,
      name: entry.name,
      missingAppId: null,
    });
  };

  const handleSortChange = (col: SortColumn) => {
    if (sortBy === col) {
      setSortDirection((prev) => (prev === "asc" ? "desc" : "asc"));
    } else {
      setSortBy(col);
      setSortDirection("asc");
    }
  };

  // Operation dialog triggers
  const handleOpenCreateDir = () => {
    setDialogError(null);
    setCreateDirOpen(true);
  };

  const handleOpenCreateFile = () => {
    setDialogError(null);
    setCreateFileOpen(true);
  };

  const handleOpenRename = () => {
    if (selectedPaths.size !== 1) return;
    setDialogError(null);
    setRenameOpen(true);
  };

  const handleOpenDelete = () => {
    if (selectedPaths.size === 0) return;
    setDialogError(null);
    setDeleteOpen(true);
  };

  // Execute Create Directory
  const handleConfirmCreateDir = async (dirName: string) => {
    setIsOperating(true);
    setDialogError(null);
    try {
      await IpcClient.createLocalDirectory(currentPath, dirName);
      setCreateDirOpen(false);
      await loadDirectory(currentPath, false);
    } catch (err: unknown) {
      const msg =
        err && typeof err === "object" && "message" in err
          ? String(err.message)
          : "Failed to create directory";
      setDialogError(msg);
    } finally {
      setIsOperating(false);
    }
  };

  // Execute Create File
  const handleConfirmCreateFile = async (fileName: string) => {
    setIsOperating(true);
    setDialogError(null);
    try {
      await IpcClient.createLocalFile(currentPath, fileName);
      setCreateFileOpen(false);
      await loadDirectory(currentPath, false);
    } catch (err: unknown) {
      const msg =
        err && typeof err === "object" && "message" in err
          ? String(err.message)
          : "Failed to create file";
      setDialogError(msg);
    } finally {
      setIsOperating(false);
    }
  };

  // Execute Rename
  const handleConfirmRename = async (newName: string) => {
    if (selectedPaths.size !== 1) return;
    const sourcePath = Array.from(selectedPaths)[0];
    setIsOperating(true);
    setDialogError(null);
    try {
      await IpcClient.renameLocalEntry(sourcePath, newName);
      setRenameOpen(false);
      await loadDirectory(currentPath, false);
    } catch (err: unknown) {
      const msg =
        err && typeof err === "object" && "message" in err
          ? String(err.message)
          : "Failed to rename entry";
      setDialogError(msg);
    } finally {
      setIsOperating(false);
    }
  };

  // Execute Delete
  const handleConfirmDelete = async () => {
    if (selectedPaths.size === 0) return;
    setIsOperating(true);
    setDialogError(null);
    try {
      await IpcClient.deleteLocalEntries(Array.from(selectedPaths));
      setDeleteOpen(false);
      setSelectedPaths(new Set());
      await loadDirectory(currentPath, false);
    } catch (err: unknown) {
      const msg =
        err && typeof err === "object" && "message" in err
          ? String(err.message)
          : "Failed to delete entries";
      setDialogError(msg);
    } finally {
      setIsOperating(false);
    }
  };

  // Context Menu trigger
  const handleContextMenu = (e: React.MouseEvent) => {
    setContextMenuPos({ x: e.clientX, y: e.clientY });
  };

  // Summary statistics & selection details
  const selectedEntries = useMemo(() => {
    return entries.filter((e) => selectedPaths.has(e.path));
  }, [entries, selectedPaths]);

  const singleSelectedEntry = selectedEntries.length === 1 ? selectedEntries[0] : null;

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

  const handleSelectionChange = useCallback((next: Set<string>, focused?: string) => {
    setSelectedPaths(next);
    setFocusedPath(focused);
  }, []);

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
    pane: "local",
    currentPath,
    isReady: Boolean(currentPath) && !isLoading && !error,
    entries,
    selectedPaths,
    onSelectionChange: handleSelectionChange,
    activeDrag,
    onInternalDragStart,
    onInternalDragEnd,
    onAcceptDrop: onDropDownload,
  });

  return (
    <section
      role="region"
      aria-label="Local Filesystem"
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
      {/* Pane Title Bar with Quick Actions */}
      <div className="flex items-center justify-between px-3 py-1.5 bg-commander-darker border-b border-commander-border text-xs font-medium select-none">
        <div className="flex items-center gap-1.5 text-commander-accent">
          <Folder className="w-3.5 h-3.5" />
          <span>Local Filesystem</span>
        </div>

        {/* Operation Toolbar Buttons */}
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={handleOpenCreateDir}
            className="p-1 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-text hover:text-white transition-colors"
            title="New Folder (F7)"
            aria-label="New Folder (F7)"
          >
            <FolderPlus className="w-3.5 h-3.5 text-amber-400" />
          </button>

          <button
            type="button"
            onClick={handleOpenCreateFile}
            className="p-1 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-text hover:text-white transition-colors"
            title="New File"
            aria-label="New File"
          >
            <FilePlus className="w-3.5 h-3.5 text-commander-accent" />
          </button>

          <button
            type="button"
            onClick={handleOpenRename}
            disabled={selectedPaths.size !== 1}
            className={`p-1 rounded border transition-colors ${
              selectedPaths.size === 1
                ? "bg-commander-dark hover:bg-commander-selected border-commander-border text-commander-text hover:text-white cursor-pointer"
                : "bg-commander-dark/40 border-commander-border/40 text-commander-subtext/30 cursor-not-allowed"
            }`}
            title="Rename Selected Item (F2)"
            aria-label="Rename Selected Item (F2)"
          >
            <Edit3 className="w-3.5 h-3.5 text-sky-400" />
          </button>

          <button
            type="button"
            onClick={handleOpenDelete}
            disabled={selectedPaths.size === 0}
            className={`p-1 rounded border transition-colors ${
              selectedPaths.size > 0
                ? "bg-rose-950/40 hover:bg-rose-900/60 border-rose-900/60 text-rose-300 cursor-pointer"
                : "bg-commander-dark/40 border-commander-border/40 text-commander-subtext/30 cursor-not-allowed"
            }`}
            title="Delete Selected Items (Del)"
            aria-label="Delete Selected Items (Del)"
          >
            <Trash2 className="w-3.5 h-3.5 text-rose-400" />
          </button>

          {onUpload && (
            <button
              type="button"
              onClick={() => onUpload(Array.from(selectedPaths))}
              disabled={!canUpload || selectedPaths.size === 0}
              className={`p-1 rounded border transition-colors ${
                canUpload && selectedPaths.size > 0
                  ? "bg-sky-950/60 hover:bg-sky-900/70 border-sky-700/60 text-sky-200 cursor-pointer"
                  : "bg-commander-dark/40 border-commander-border/40 text-commander-subtext/30 cursor-not-allowed"
              }`}
              title="Upload Selected to Remote Pane"
              aria-label="Upload Selected to Remote Pane"
            >
              <Upload className="w-3.5 h-3.5 text-sky-400" />
            </button>
          )}
        </div>
      </div>

      {/* Address & Navigation Bar */}
      <AddressBar
        path={currentPath}
        paneType="local"
        onNavigate={handleNavigate}
        onBack={handleBack}
        canGoBack={historyIndex > 0}
        onForward={handleForward}
        canGoForward={historyIndex < history.length - 1}
        onUp={handleUp}
        canGoUp={!!parentPath && parentPath !== currentPath}
        onHome={handleHome}
        onRoot={handleRoot}
        onRefresh={handleRefresh}
        showHidden={showHidden}
        onToggleHidden={() => setShowHidden((prev) => !prev)}
        isLoading={isLoading}
      />

      {/* Error Banner */}
      {error && (
        <div className="flex items-center justify-between p-2 bg-rose-950/80 border-b border-rose-800 text-rose-200 text-xs">
          <div className="flex items-center gap-1.5">
            <AlertCircle className="w-4 h-4 shrink-0 text-rose-400" />
            <span>{error}</span>
          </div>
          <div className="flex items-center gap-2">
            {parentPath && (
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

      {/* File Table */}
      <FileTable
        entries={entries}
        selectedPaths={selectedPaths}
        onSelectionChange={handleSelectionChange}
        focusedPath={focusedPath}
        onOpenEntry={handleOpenEntry}
        onNavigateParent={handleUp}
        sortBy={sortBy}
        sortDirection={sortDirection}
        onSortChange={handleSortChange}
        isLoading={isLoading}
        onContextMenu={handleContextMenu}
        onDeleteSelected={handleOpenDelete}
        onRenameSelected={handleOpenRename}
        onNewFolder={handleOpenCreateDir}
        onEntryDragStart={handleEntryDragStart}
        onEntryDragEnd={handleEntryDragEnd}
        onRowDragOver={handleRowDragOver}
        onRowDragLeave={handleRowDragLeave}
        onRowDrop={handleRowDrop}
        dropHighlightPath={dropState.status !== "idle" ? dropState.hoveredEntryPath : null}
        dropHighlightKind={dropState.status !== "idle" ? dropState.status : null}
      />

      {/* Internal Drop Target Indicator (Bottom of Pane) */}
      <PaneDropIndicator dropState={dropState} itemCount={activeDrag?.sourcePaths.length ?? 0} />

      {/* Pane Status / Summary Bar */}
      <div className="flex items-center justify-between px-3 py-1 bg-commander-darker border-t border-commander-border text-[11px] text-commander-subtext select-none">
        <div>
          <span>{summary.totalCount} items</span>
          {summary.selectedCount > 0 && (
            <span className="ml-2 font-medium text-commander-accent">
              ({summary.selectedCount} selected: {formatFileSize(summary.selectedBytes, false)})
            </span>
          )}
        </div>
        <div className="text-[10px]">
          <span>{showHidden ? "Hidden files visible" : "Hidden files hidden"}</span>
        </div>
      </div>

      {/* Context Menu */}
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
        onNewFolder={handleOpenCreateDir}
        onNewFile={handleOpenCreateFile}
        onRename={singleSelectedEntry ? handleOpenRename : undefined}
        onDelete={handleOpenDelete}
        onRefresh={handleRefresh}
      />

      {/* Open With Application Picker Dialog */}
      <OpenWithDialog
        isOpen={Boolean(openWithTarget)}
        filePath={openWithTarget?.path ?? null}
        fileName={openWithTarget?.name ?? null}
        missingRememberedAppId={openWithTarget?.missingAppId}
        onClose={() => setOpenWithTarget(null)}
      />

      {/* Create Directory Dialog */}
      <InputDialog
        isOpen={createDirOpen}
        title="Create New Folder"
        description={`Enter folder name to create in: ${currentPath}`}
        placeholder="Folder name"
        confirmLabel="Create Folder"
        isLoading={isOperating}
        error={dialogError}
        onConfirm={handleConfirmCreateDir}
        onCancel={() => setCreateDirOpen(false)}
      />

      {/* Create File Dialog */}
      <InputDialog
        isOpen={createFileOpen}
        title="Create New File"
        description={`Enter file name to create in: ${currentPath}`}
        placeholder="example.txt"
        confirmLabel="Create File"
        isLoading={isOperating}
        error={dialogError}
        onConfirm={handleConfirmCreateFile}
        onCancel={() => setCreateFileOpen(false)}
      />

      {/* Rename Dialog */}
      <InputDialog
        isOpen={renameOpen}
        title="Rename Item"
        description={`Enter new name for: ${singleSelectedEntry?.name || ""}`}
        initialValue={singleSelectedEntry?.name || ""}
        placeholder="New name"
        confirmLabel="Rename"
        isLoading={isOperating}
        error={dialogError}
        onConfirm={handleConfirmRename}
        onCancel={() => setRenameOpen(false)}
      />

      {/* Delete Confirmation Dialog */}
      <ConfirmDialog
        isOpen={deleteOpen}
        title="Confirm Permanent Deletion"
        description={`Are you sure you want to permanently delete the selected item(s)? This action cannot be undone.`}
        items={selectedEntries.map((e) => e.name)}
        confirmLabel="Delete Permanently"
        isDestructive={true}
        isLoading={isOperating}
        error={dialogError}
        onConfirm={handleConfirmDelete}
        onCancel={() => setDeleteOpen(false)}
      />
    </section>
  );
};
