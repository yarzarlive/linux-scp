import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TransferQueue } from "../../src/components/TransferQueue/TransferQueue";
import { TransferJob } from "../../src/types/transfer";

const SAMPLE_JOBS: TransferJob[] = [
  {
    id: "job-1",
    session_id: "sess-1",
    source_path: "/home/user/archive.tar.gz",
    destination_path: "/var/www/archive.tar.gz",
    direction: "upload",
    state: { status: "Transferring" },
    total_bytes: 2048,
    transferred_bytes: 1024,
    speed_bps: 512,
    eta_seconds: 2,
    conflict_policy: "ask",
    is_directory: true,
    total_files: 3,
    completed_files: 1,
    created_at_ms: 1700000000000,
    updated_at_ms: 1700000001000,
  },
  {
    id: "job-2",
    session_id: "sess-1",
    source_path: "/var/www/logs",
    destination_path: "/home/user/logs",
    direction: "download",
    state: { status: "Failed", details: { error_message: "Permission denied" } },
    total_bytes: 4096,
    transferred_bytes: 1024,
    speed_bps: 0,
    conflict_policy: "ask",
    is_directory: true,
    total_files: 3,
    completed_files: 1,
    created_at_ms: 1700000000000,
    updated_at_ms: 1700000002000,
  },
  {
    id: "job-3",
    session_id: "sess-1",
    source_path: "/home/user/readme.md",
    destination_path: "/var/www/readme.md",
    direction: "upload",
    state: { status: "Completed" },
    total_bytes: 512,
    transferred_bytes: 512,
    speed_bps: 0,
    eta_seconds: 0,
    conflict_policy: "overwrite",
    is_directory: false,
    total_files: 1,
    completed_files: 1,
    created_at_ms: 1700000000000,
    updated_at_ms: 1700000003000,
  },
];

describe("TransferQueue Component", () => {
  it("renders idle transfer queue summary and tabs", () => {
    render(<TransferQueue height={112} isCollapsed={false} onToggleCollapse={vi.fn()} />);

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
      <TransferQueue height={112} isCollapsed={false} onToggleCollapse={handleToggleCollapse} />
    );

    fireEvent.click(screen.getByTitle("Collapse Transfer Queue"));
    expect(handleToggleCollapse).toHaveBeenCalledTimes(1);

    rerender(
      <TransferQueue height={112} isCollapsed={true} onToggleCollapse={handleToggleCollapse} />
    );
    expect(screen.queryByText("Transfer queue is idle")).not.toBeInTheDocument();
    expect(screen.getByTitle("Expand Transfer Queue")).toBeInTheDocument();
  });

  it("renders active, failed, and completed jobs with progress, speed, ETA, cancel, and retry", () => {
    const onCancel = vi.fn();
    const onRetry = vi.fn();
    const onClear = vi.fn();

    render(
      <TransferQueue
        jobs={SAMPLE_JOBS}
        height={200}
        isCollapsed={false}
        onToggleCollapse={vi.fn()}
        onCancelJob={onCancel}
        onRetryJob={onRetry}
        onClearFinished={onClear}
      />
    );

    expect(screen.getByText("All (3)")).toBeInTheDocument();
    expect(screen.getByText("Active (1)")).toBeInTheDocument();
    expect(screen.getByText("Completed (1)")).toBeInTheDocument();

    expect(screen.getByText("50%")).toBeInTheDocument();
    expect(screen.getByText("ETA: 2s")).toBeInTheDocument();
    expect(screen.getByText("Permission denied")).toBeInTheDocument();
    expect(screen.getByText("1/3 files")).toBeInTheDocument();

    fireEvent.click(screen.getByTitle("Cancel Transfer"));
    expect(onCancel).toHaveBeenCalledWith("job-1");

    fireEvent.click(screen.getByTitle("Retry Transfer"));
    expect(onRetry).toHaveBeenCalledWith("job-2");

    fireEvent.click(screen.getByTitle("Clear Completed & Cancelled"));
    expect(onClear).toHaveBeenCalledTimes(1);
  });
});
