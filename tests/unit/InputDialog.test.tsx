import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { InputDialog } from "../../src/components/Common/InputDialog";

describe("InputDialog Component", () => {
  it("renders when open and handles confirm", () => {
    const handleConfirm = vi.fn();
    const handleCancel = vi.fn();

    render(
      <InputDialog
        isOpen={true}
        title="Create New Folder"
        description="Enter name"
        initialValue="New Folder"
        onConfirm={handleConfirm}
        onCancel={handleCancel}
      />
    );

    expect(screen.getByRole("heading", { name: "Create New Folder" })).toBeInTheDocument();
    const input = screen.getByRole("textbox", { name: "Create New Folder" }) as HTMLInputElement;
    expect(input.value).toBe("New Folder");

    fireEvent.change(input, { target: { value: "My Projects" } });
    fireEvent.submit(input);

    expect(handleConfirm).toHaveBeenCalledWith("My Projects");
  });

  it("handles cancel button click", () => {
    const handleCancel = vi.fn();
    render(<InputDialog isOpen={true} title="Test" onConfirm={vi.fn()} onCancel={handleCancel} />);

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(handleCancel).toHaveBeenCalledTimes(1);
  });

  it("does not render when isOpen is false", () => {
    render(
      <InputDialog isOpen={false} title="Hidden Dialog" onConfirm={vi.fn()} onCancel={vi.fn()} />
    );

    expect(screen.queryByText("Hidden Dialog")).not.toBeInTheDocument();
  });
});
