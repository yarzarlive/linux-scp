import React from "react";
import { ChevronUp, ChevronDown } from "lucide-react";

export type SortColumn = "name" | "size" | "changed" | "rights" | "owner";
export type SortDirection = "asc" | "desc";

interface ColumnHeaderProps {
  sortBy?: SortColumn | null;
  sortDirection?: SortDirection;
  onSortChange?: (column: SortColumn) => void;
  disabled?: boolean;
}

export const ColumnHeader: React.FC<ColumnHeaderProps> = ({
  sortBy,
  sortDirection = "asc",
  onSortChange,
  disabled = false,
}) => {
  const columns: { id: SortColumn; label: string; width: string; align?: "left" | "right" }[] = [
    { id: "name", label: "Name", width: "flex-1 min-w-[160px]", align: "left" },
    { id: "size", label: "Size", width: "w-24 min-w-[6rem]", align: "right" },
    { id: "changed", label: "Changed", width: "w-36 min-w-[9rem]", align: "left" },
    { id: "rights", label: "Rights", width: "w-28 min-w-[7rem]", align: "left" },
    { id: "owner", label: "Owner", width: "w-28 min-w-[7rem]", align: "left" },
  ];

  return (
    <div
      role="row"
      className="flex items-center bg-commander-darker border-b border-commander-border text-[11px] font-semibold text-commander-subtext select-none sticky top-0 z-10"
    >
      {columns.map((col) => {
        const isCurrentSort = sortBy === col.id;
        return (
          <button
            key={col.id}
            type="button"
            disabled={disabled}
            onClick={() => onSortChange?.(col.id)}
            className={`flex items-center px-2 py-1.5 border-r border-commander-border/50 hover:bg-commander-dark hover:text-commander-text transition-colors text-left ${
              col.width
            } ${col.align === "right" ? "justify-end" : "justify-between"} ${
              disabled ? "cursor-default opacity-60" : "cursor-pointer"
            }`}
            title={`Sort by ${col.label}`}
          >
            <span className={col.align === "right" ? "order-2 ml-1" : "order-1"}>{col.label}</span>
            <span
              className={`flex items-center text-commander-accent ${
                col.align === "right" ? "order-1 mr-1" : "order-2"
              }`}
            >
              {isCurrentSort ? (
                sortDirection === "asc" ? (
                  <ChevronUp className="w-3.5 h-3.5" data-testid="sort-asc" />
                ) : (
                  <ChevronDown className="w-3.5 h-3.5" data-testid="sort-desc" />
                )
              ) : (
                <span className="w-3.5 h-3.5" />
              )}
            </span>
          </button>
        );
      })}
    </div>
  );
};
