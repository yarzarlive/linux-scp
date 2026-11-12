import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ConfirmDialog } from "../../src/components/Common/ConfirmDialog";

describe("ConfirmDialog Component", () => {
  it("renders target items and handles confirm", () => {
    const handleConfirm = vi.fn();
    const handleCancel = vi.fn();

    render(
      <ConfirmDialog
        isOpen={true}
        title="Confirm Permanent Deletion"
        description="Are you sure?"
        items={["file1.txt", "dir2"]}
        confirmLabel="Delete Now"
        isDestructive={true}
        onConfirm={handleConfirm}
        onCancel={handleCancel}
      />
    );

    expect(screen.getByText("Confirm Permanent Deletion")).toBeInTheDocument();
    expect(screen.getByText("file1.txt")).toBeInTheDocument();
    expect(screen.getByText("dir2")).toBeInTheDocument();

    const deleteBtn = screen.getByRole("button", { name: "Delete Now" });
    fireEvent.click(deleteBtn);

    expect(handleConfirm).toHaveBeenCalledTimes(1);
  });

  it("handles cancel button click", () => {
    const handleCancel = vi.fn();
    render(
      <ConfirmDialog
        isOpen={true}
        title="Confirm"
        description="Desc"
        onConfirm={vi.fn()}
        onCancel={handleCancel}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(handleCancel).toHaveBeenCalledTimes(1);
  });
});

