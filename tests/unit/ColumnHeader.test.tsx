import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ColumnHeader } from "../../src/components/FilePanel/ColumnHeader";

describe("ColumnHeader Component", () => {
  it("renders all five required columns in exact specification order", () => {
    render(<ColumnHeader sortBy="name" sortDirection="asc" />);

    const buttons = screen.getAllByRole("button");
    expect(buttons).toHaveLength(5);

    expect(buttons[0]).toHaveTextContent("Name");
    expect(buttons[1]).toHaveTextContent("Size");
    expect(buttons[2]).toHaveTextContent("Changed");
    expect(buttons[3]).toHaveTextContent("Rights");
    expect(buttons[4]).toHaveTextContent("Owner");
  });

  it("calls onSortChange when a column header is clicked", () => {
    const handleSortChange = vi.fn();
    render(
      <ColumnHeader sortBy="name" sortDirection="asc" onSortChange={handleSortChange} />
    );

    const sizeButton = screen.getByTitle("Sort by Size");
    fireEvent.click(sizeButton);

    expect(handleSortChange).toHaveBeenCalledWith("size");
  });

  it("displays sort direction indicators", () => {
    const { rerender } = render(
      <ColumnHeader sortBy="name" sortDirection="asc" />
    );
    expect(screen.getByTestId("sort-asc")).toBeInTheDocument();

    rerender(<ColumnHeader sortBy="name" sortDirection="desc" />);
    expect(screen.getByTestId("sort-desc")).toBeInTheDocument();
  });
});
