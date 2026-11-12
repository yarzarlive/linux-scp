import React, { useEffect, useRef } from "react";
import { FolderPlus, FilePlus, Edit3, Trash2, RefreshCw, FolderOpen } from "lucide-react";

export interface ContextMenuPosition {
  x: number;
  y: number;
}

interface ContextMenuProps {
  position: ContextMenuPosition | null;
  onClose: () => void;
  hasSelection: boolean;
  isSingleSelection: boolean;
  isDirectorySelection: boolean;
  onOpen?: () => void;
  onNewFolder?: () => void;
  onNewFile?: () => void;
  onRename?: () => void;
  onDelete?: () => void;
  onRefresh?: () => void;
}

export const ContextMenu: React.FC<ContextMenuProps> = ({
  position,
  onClose,
  hasSelection,
  isSingleSelection,
  isDirectorySelection,
  onOpen,
  onNewFolder,
  onNewFile,
  onRename,
  onDelete,
  onRefresh,
}) => {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!position) return;

    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };

    window.addEventListener("mousedown", handleClickOutside);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("mousedown", handleClickOutside);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [position, onClose]);

  if (!position) return null;

  return (
    <div
      ref={menuRef}
      style={{ top: position.y, left: position.x }}
      className="fixed z-50 min-w-[160px] py-1 bg-commander-surface border border-commander-border rounded shadow-xl text-xs text-commander-text select-none animate-in fade-in-50 duration-75"
      role="menu"
      aria-label="Filesystem Context Menu"
    >
      {hasSelection && (
        <>
          {isDirectorySelection && onOpen && (
            <button
              type="button"
              onClick={() => {
                onOpen();
                onClose();
              }}
              className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-commander-dark text-left transition-colors"
              role="menuitem"
            >
              <FolderOpen className="w-3.5 h-3.5 text-amber-400" />
              <span>Open</span>
            </button>
          )}

          {isSingleSelection && onRename && (
            <button
              type="button"
              onClick={() => {
                onRename();
                onClose();
              }}
              className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-commander-dark text-left transition-colors"
              role="menuitem"
            >
              <Edit3 className="w-3.5 h-3.5 text-sky-400" />
              <span>Rename (F2)</span>
            </button>
          )}

          {onDelete && (
            <button
              type="button"
              onClick={() => {
                onDelete();
                onClose();
              }}
              className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-rose-950/50 text-rose-300 text-left transition-colors"
              role="menuitem"
            >
              <Trash2 className="w-3.5 h-3.5 text-rose-400" />
              <span>Delete (Del)</span>
            </button>
          )}

          <div className="h-px bg-commander-border my-1" />
        </>
      )}

      {onNewFolder && (
        <button
          type="button"
          onClick={() => {
            onNewFolder();
            onClose();
          }}
          className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-commander-dark text-left transition-colors"
          role="menuitem"
        >
          <FolderPlus className="w-3.5 h-3.5 text-amber-400" />
          <span>New Folder (F7)</span>
        </button>
      )}

      {onNewFile && (
        <button
          type="button"
          onClick={() => {
            onNewFile();
            onClose();
          }}
          className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-commander-dark text-left transition-colors"
          role="menuitem"
        >
          <FilePlus className="w-3.5 h-3.5 text-commander-accent" />
          <span>New File</span>
        </button>
      )}

      {onRefresh && (
        <button
          type="button"
          onClick={() => {
            onRefresh();
            onClose();
          }}
          className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-commander-dark text-left transition-colors"
          role="menuitem"
        >
          <RefreshCw className="w-3.5 h-3.5 text-commander-subtext" />
          <span>Refresh (F5)</span>
        </button>
      )}
    </div>
  );
};
