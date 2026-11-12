import React from "react";
import { Folder, FileText, CornerDownRight, FileQuestion } from "lucide-react";
import { FileEntry } from "../../types/filesystem";
import {
  formatFileSize,
  formatOwnership,
  formatPermissions,
  formatTimestamp,
  isDirectory,
  isSymlink,
} from "../../utils/formatters";

interface FileRowProps {
  entry: FileEntry;
  isSelected: boolean;
  isFocused?: boolean;
  draggable?: boolean;
  dropHighlight?: "valid" | "invalid" | null;
  onClick: (e: React.MouseEvent, entry: FileEntry) => void;
  onDoubleClick: (entry: FileEntry) => void;
  onContextMenu?: (e: React.MouseEvent, entry: FileEntry) => void;
  onDragStart?: (e: React.DragEvent, entry: FileEntry) => void;
  onDragEnd?: (e: React.DragEvent) => void;
  onDragOver?: (e: React.DragEvent, entry: FileEntry) => void;
  onDragLeave?: (e: React.DragEvent, entry: FileEntry) => void;
  onDrop?: (e: React.DragEvent, entry: FileEntry) => void;
}

export const FileRow: React.FC<FileRowProps> = ({
  entry,
  isSelected,
  isFocused = false,
  draggable = false,
  dropHighlight = null,
  onClick,
  onDoubleClick,
  onContextMenu,
  onDragStart,
  onDragEnd,
  onDragOver,
  onDragLeave,
  onDrop,
}) => {
  const isDir = isDirectory(entry.file_type);
  const isSym = isSymlink(entry.file_type);

  const renderIcon = () => {
    if (isDir) {
      return <Folder className="w-4 h-4 text-amber-400 shrink-0" />;
    }
    if (isSym) {
      return <CornerDownRight className="w-4 h-4 text-sky-400 shrink-0" />;
    }
    if (entry.file_type.type === "Special") {
      return <FileQuestion className="w-4 h-4 text-purple-400 shrink-0" />;
    }
    return <FileText className="w-4 h-4 text-commander-subtext shrink-0" />;
  };

  const symlinkTarget =
    entry.file_type.type === "Symlink" && typeof entry.file_type.target === "string"
      ? entry.file_type.target
      : undefined;

  return (
    <div
      role="row"
      aria-selected={isSelected}
      draggable={draggable}
      data-drop-highlight={dropHighlight ?? undefined}
      onClick={(e) => onClick(e, entry)}
      onDoubleClick={() => onDoubleClick(entry)}
      onContextMenu={(e) => onContextMenu?.(e, entry)}
      onDragStart={(e) => onDragStart?.(e, entry)}
      onDragEnd={(e) => onDragEnd?.(e)}
      onDragOver={(e) => onDragOver?.(e, entry)}
      onDragLeave={(e) => onDragLeave?.(e, entry)}
      onDrop={(e) => onDrop?.(e, entry)}
      className={`flex items-center text-xs border-b border-commander-border/30 select-none cursor-pointer transition-colors ${
        dropHighlight === "valid"
          ? "bg-emerald-950/60 ring-1 ring-inset ring-emerald-400 text-white"
          : dropHighlight === "invalid"
            ? "bg-rose-950/40 ring-1 ring-inset ring-rose-500/80"
            : isSelected
              ? "bg-commander-selected/80 text-white font-medium"
              : "hover:bg-commander-dark/60 text-commander-text"
      } ${isFocused ? "outline outline-1 outline-commander-accent -outline-offset-1" : ""}`}
    >
      {/* 1. Name */}
      <div className="flex-1 min-w-[160px] flex items-center gap-2 px-2 py-1.5 overflow-hidden border-r border-commander-border/30">
        {renderIcon()}
        <span className="truncate" title={entry.name}>
          {entry.name}
        </span>
        {symlinkTarget && (
          <span
            className="text-[10px] text-commander-subtext/70 truncate shrink italic"
            title={`points to ${symlinkTarget}`}
          >
            → {symlinkTarget}
          </span>
        )}
      </div>

      {/* 2. Size */}
      <div className="w-24 min-w-[6rem] px-2 py-1.5 text-right font-mono text-[11px] text-commander-subtext border-r border-commander-border/30">
        {formatFileSize(entry.metadata.size, isDir)}
      </div>

      {/* 3. Changed */}
      <div className="w-36 min-w-[9rem] px-2 py-1.5 font-mono text-[11px] text-commander-subtext truncate border-r border-commander-border/30">
        {formatTimestamp(entry.metadata.modified_at_secs)}
      </div>

      {/* 4. Rights */}
      <div className="w-28 min-w-[7rem] px-2 py-1.5 font-mono text-[11px] text-commander-subtext tracking-wider border-r border-commander-border/30">
        {formatPermissions(entry.file_type, entry.metadata.permissions)}
      </div>

      {/* 5. Owner */}
      <div className="w-28 min-w-[7rem] px-2 py-1.5 font-mono text-[11px] text-commander-subtext truncate">
        {formatOwnership(entry.metadata.ownership)}
      </div>
    </div>
  );
};
