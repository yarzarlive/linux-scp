import React, { useState, useEffect, useCallback, useMemo } from "react";
import { Folder, AlertCircle } from "lucide-react";
import { FileEntry } from "../../types/filesystem";
import { IpcClient } from "../../services/ipc";
import { AddressBar } from "./AddressBar";
import { FileTable } from "./FileTable";
import { SortColumn, SortDirection } from "./ColumnHeader";
import { formatFileSize, isDirectory } from "../../utils/formatters";

interface LocalPaneProps {
  initialPath?: string;
  onDirectoryChange?: (path: string) => void;
  isActivePane?: boolean;
  onFocusPane?: () => void;
}

export const LocalPane: React.FC<LocalPaneProps> = ({
  initialPath,
  onDirectoryChange,
  isActivePane = true,
  onFocusPane,
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

  const handleOpenEntry = (entry: FileEntry) => {
    if (isDirectory(entry.file_type)) {
      loadDirectory(entry.path, true);
    }
  };

  const handleSortChange = (col: SortColumn) => {
    if (sortBy === col) {
      setSortDirection((prev) => (prev === "asc" ? "desc" : "asc"));
    } else {
      setSortBy(col);
      setSortDirection("asc");
    }
  };

  // Summary statistics
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

  return (
    <section
      role="region"
      aria-label="Local Filesystem"
      onClick={onFocusPane}
      className={`flex flex-col h-full rounded border overflow-hidden bg-commander-surface transition-colors ${
        isActivePane ? "border-commander-accent/50" : "border-commander-border"
      }`}
    >
      {/* Pane Title Bar */}
      <div className="flex items-center justify-between px-3 py-1.5 bg-commander-darker border-b border-commander-border text-xs font-medium select-none">
        <div className="flex items-center gap-1.5 text-commander-accent">
          <Folder className="w-3.5 h-3.5" />
          <span>Local Filesystem</span>
        </div>
        <span className="text-[10px] text-commander-subtext font-mono">
          {currentPath || "Loading..."}
        </span>
      </div>

      {/* Address & Navigation Bar */}
      <AddressBar
        path={currentPath}
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
        onSelectionChange={(next, focused) => {
          setSelectedPaths(next);
          setFocusedPath(focused);
        }}
        focusedPath={focusedPath}
        onOpenEntry={handleOpenEntry}
        onNavigateParent={handleUp}
        sortBy={sortBy}
        sortDirection={sortDirection}
        onSortChange={handleSortChange}
        isLoading={isLoading}
      />

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
    </section>
  );
};
