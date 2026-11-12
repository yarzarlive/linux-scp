import React, { useRef, useMemo } from "react";
import { FileEntry } from "../../types/filesystem";
import { ColumnHeader, SortColumn, SortDirection } from "./ColumnHeader";
import { FileRow } from "./FileRow";
import { isDirectory } from "../../utils/formatters";

interface FileTableProps {
  entries: FileEntry[];
  selectedPaths: Set<string>;
  onSelectionChange: (selectedPaths: Set<string>, focusedPath?: string) => void;
  focusedPath?: string;
  onOpenEntry: (entry: FileEntry) => void;
  onNavigateParent?: () => void;
  sortBy: SortColumn;
  sortDirection: SortDirection;
  onSortChange: (column: SortColumn) => void;
  isLoading?: boolean;
  onContextMenu?: (e: React.MouseEvent, entry?: FileEntry) => void;
  onDeleteSelected?: () => void;
  onRenameSelected?: () => void;
  onNewFolder?: () => void;
}

export const FileTable: React.FC<FileTableProps> = ({
  entries,
  selectedPaths,
  onSelectionChange,
  focusedPath,
  onOpenEntry,
  onNavigateParent,
  sortBy,
  sortDirection,
  onSortChange,
  isLoading = false,
  onContextMenu,
  onDeleteSelected,
  onRenameSelected,
  onNewFolder,
}) => {
  const tableRef = useRef<HTMLDivElement>(null);
  const lastClickedPathRef = useRef<string | null>(null);

  // Sort entries: directories first, then files, sorted by chosen column
  const sortedEntries = useMemo(() => {
    const list = [...entries];

    list.sort((a, b) => {
      const aIsDir = isDirectory(a.file_type);
      const bIsDir = isDirectory(b.file_type);

      // Keep directories grouped on top
      if (aIsDir && !bIsDir) return -1;
      if (!aIsDir && bIsDir) return 1;

      let comparison = 0;
      switch (sortBy) {
        case "name":
          comparison = a.name.localeCompare(b.name, undefined, {
            numeric: true,
            sensitivity: "base",
          });
          break;
        case "size": {
          const aSize = a.metadata.size ?? -1;
          const bSize = b.metadata.size ?? -1;
          comparison = aSize - bSize;
          break;
        }
        case "changed": {
          const aTime = a.metadata.modified_at_secs ?? 0;
          const bTime = b.metadata.modified_at_secs ?? 0;
          comparison = aTime - bTime;
          break;
        }
        case "rights": {
          const aMode = a.metadata.permissions?.raw_mode ?? 0;
          const bMode = b.metadata.permissions?.raw_mode ?? 0;
          comparison = aMode - bMode;
          break;
        }
        case "owner": {
          const aOwner = a.metadata.ownership?.user_name ?? String(a.metadata.ownership?.uid ?? "");
          const bOwner = b.metadata.ownership?.user_name ?? String(b.metadata.ownership?.uid ?? "");
          comparison = aOwner.localeCompare(bOwner);
          break;
        }
      }

      return sortDirection === "asc" ? comparison : -comparison;
    });

    return list;
  }, [entries, sortBy, sortDirection]);

  // Handle single, Ctrl, and Shift selection
  const handleRowClick = (e: React.MouseEvent, entry: FileEntry) => {
    tableRef.current?.focus();

    if (e.ctrlKey || e.metaKey) {
      // Toggle single item
      const next = new Set(selectedPaths);
      if (next.has(entry.path)) {
        next.delete(entry.path);
      } else {
        next.add(entry.path);
      }
      lastClickedPathRef.current = entry.path;
      onSelectionChange(next, entry.path);
    } else if (e.shiftKey && lastClickedPathRef.current) {
      // Range selection
      const fromIndex = sortedEntries.findIndex((item) => item.path === lastClickedPathRef.current);
      const toIndex = sortedEntries.findIndex((item) => item.path === entry.path);

      if (fromIndex !== -1 && toIndex !== -1) {
        const start = Math.min(fromIndex, toIndex);
        const end = Math.max(fromIndex, toIndex);
        const next = new Set(selectedPaths);
        for (let i = start; i <= end; i++) {
          next.add(sortedEntries[i].path);
        }
        onSelectionChange(next, entry.path);
      }
    } else {
      // Normal single selection
      lastClickedPathRef.current = entry.path;
      onSelectionChange(new Set([entry.path]), entry.path);
    }
  };

  const handleRowContextMenu = (e: React.MouseEvent, entry: FileEntry) => {
    e.preventDefault();
    e.stopPropagation();

    // If clicked item is not already part of current selection, select it
    if (!selectedPaths.has(entry.path)) {
      lastClickedPathRef.current = entry.path;
      onSelectionChange(new Set([entry.path]), entry.path);
    }

    onContextMenu?.(e, entry);
  };

  const handleContainerContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    onContextMenu?.(e, undefined);
  };

  // Keyboard navigation
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (sortedEntries.length === 0) return;

    const currentIndex = sortedEntries.findIndex((item) => item.path === focusedPath);

    if (e.key === "ArrowDown") {
      e.preventDefault();
      const nextIndex = currentIndex < sortedEntries.length - 1 ? currentIndex + 1 : currentIndex;
      const nextEntry = sortedEntries[nextIndex];
      lastClickedPathRef.current = nextEntry.path;
      onSelectionChange(new Set([nextEntry.path]), nextEntry.path);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      const prevIndex = currentIndex > 0 ? currentIndex - 1 : 0;
      const prevEntry = sortedEntries[prevIndex];
      lastClickedPathRef.current = prevEntry.path;
      onSelectionChange(new Set([prevEntry.path]), prevEntry.path);
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (currentIndex >= 0) {
        onOpenEntry(sortedEntries[currentIndex]);
      }
    } else if (e.key === "Backspace") {
      e.preventDefault();
      onNavigateParent?.();
    } else if (e.key === " ") {
      e.preventDefault();
      if (currentIndex >= 0) {
        const entry = sortedEntries[currentIndex];
        const next = new Set(selectedPaths);
        if (next.has(entry.path)) {
          next.delete(entry.path);
        } else {
          next.add(entry.path);
        }
        onSelectionChange(next, entry.path);
      }
    } else if (e.key === "Delete") {
      e.preventDefault();
      if (selectedPaths.size > 0) {
        onDeleteSelected?.();
      }
    } else if (e.key === "F2") {
      e.preventDefault();
      if (selectedPaths.size === 1) {
        onRenameSelected?.();
      }
    } else if (e.key === "F7") {
      e.preventDefault();
      onNewFolder?.();
    }
  };

  return (
    <div
      ref={tableRef}
      tabIndex={0}
      onKeyDown={handleKeyDown}
      onContextMenu={handleContainerContextMenu}
      className="flex-1 flex flex-col min-h-0 bg-commander-surface focus:outline-none overflow-hidden select-none"
      role="region"
      aria-label="Files list"
    >
      <ColumnHeader
        sortBy={sortBy}
        sortDirection={sortDirection}
        onSortChange={onSortChange}
        disabled={isLoading}
      />

      <div className="flex-1 overflow-y-auto overflow-x-hidden min-h-0">
        {sortedEntries.length === 0 ? (
          <div className="h-full flex items-center justify-center p-8 text-commander-subtext/60 text-xs">
            {isLoading ? "Reading directory..." : "Directory is empty"}
          </div>
        ) : (
          sortedEntries.map((entry) => (
            <FileRow
              key={entry.path}
              entry={entry}
              isSelected={selectedPaths.has(entry.path)}
              isFocused={focusedPath === entry.path}
              onClick={handleRowClick}
              onDoubleClick={onOpenEntry}
              onContextMenu={handleRowContextMenu}
            />
          ))
        )}
      </div>
    </div>
  );
};
