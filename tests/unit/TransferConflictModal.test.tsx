import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TransferConflictModal } from "../../src/components/TransferQueue/TransferConflictModal";
import { TransferConflictPrompt } from "../../src/types/transfer";

describe("TransferConflictModal Component", () => {
  const prompt: TransferConflictPrompt = {
    conflict_id: "conf-1",
    job_id: "job-1",
    direction: "upload",
    source_path: "/home/user/report.pdf",
    destination_path: "/var/www/report.pdf",
    source_size: 2048,
    source_modified_at_secs: 1700000200,
    target_size: 1024,
    target_modified_at_secs: 1700000100,
  };

  it("renders source and destination metadata and dispatches explicit conflict choice", () => {
    const onResolve = vi.fn();
    render(<TransferConflictModal conflict={prompt} onResolve={onResolve} />);

    expect(screen.getByText("Destination File Already Exists")).toBeInTheDocument();
    expect(screen.getByText("/home/user/report.pdf")).toBeInTheDocument();
    expect(screen.getByText("/var/www/report.pdf")).toBeInTheDocument();
    expect(screen.getByText("Size: 2.0 KB")).toBeInTheDocument();
    expect(screen.getByText("Size: 1.0 KB")).toBeInTheDocument();

    fireEvent.click(screen.getByLabelText(/Apply this choice to all remaining conflicts/i));
    fireEvent.click(screen.getByRole("button", { name: "Overwrite" }));

    expect(onResolve).toHaveBeenCalledWith("conf-1", "overwrite", true);
  });
});
