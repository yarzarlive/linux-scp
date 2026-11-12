import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TransferQueue } from "../../src/components/TransferQueue/TransferQueue";

describe("TransferQueue Component", () => {
  it("renders idle transfer queue summary and tabs", () => {
    render(
      <TransferQueue
        height={112}
        isCollapsed={false}
        onToggleCollapse={vi.fn()}
      />
    );

    expect(screen.getByText("Transfer Queue")).toBeInTheDocument();
    expect(screen.getByText("Transfer queue is idle")).toBeInTheDocument();
    expect(screen.getByText("All (0)")).toBeInTheDocument();
    expect(screen.getByText("Active (0)")).toBeInTheDocument();
    expect(screen.getByText("Queued (0)")).toBeInTheDocument();
    expect(screen.getByText("Completed (0)")).toBeInTheDocument();
  });

  it("toggles collapse state on header click", () => {
    const handleToggleCollapse = vi.fn();
    const { rerender } = render(
      <TransferQueue
        height={112}
        isCollapsed={false}
        onToggleCollapse={handleToggleCollapse}
      />
    );

    const toggleBtn = screen.getByTitle("Collapse Transfer Queue");
    fireEvent.click(toggleBtn);
    expect(handleToggleCollapse).toHaveBeenCalledTimes(1);

    rerender(
      <TransferQueue
        height={112}
        isCollapsed={true}
        onToggleCollapse={handleToggleCollapse}
      />
    );
    expect(screen.queryByText("Transfer queue is idle")).not.toBeInTheDocument();
    expect(screen.getByTitle("Expand Transfer Queue")).toBeInTheDocument();
  });
});
