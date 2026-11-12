import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RemotePane } from "../../src/components/FilePanel/RemotePane";

describe("RemotePane Component", () => {
  it("renders explicit honest disconnected state with no fabricated files", () => {
    render(<RemotePane />);

    // Shows Disconnected badge and status
    expect(screen.getAllByText("Disconnected").length).toBeGreaterThan(0);
    expect(
      screen.getByText("No Remote Connection Established")
    ).toBeInTheDocument();

    // Shows disabled sftp address bar
    const addressInput = screen.getByLabelText("Path location") as HTMLInputElement;
    expect(addressInput.value).toBe("sftp://[not connected]/");
    expect(addressInput).toBeDisabled();

    // Verifies all 5 column headers exist in disabled mode
    expect(screen.getByTitle("Sort by Name")).toBeDisabled();
    expect(screen.getByTitle("Sort by Size")).toBeDisabled();
    expect(screen.getByTitle("Sort by Changed")).toBeDisabled();
    expect(screen.getByTitle("Sort by Rights")).toBeDisabled();
    expect(screen.getByTitle("Sort by Owner")).toBeDisabled();
  });

  it("calls onConnect when New Connection button is clicked", () => {
    const handleConnect = vi.fn();
    render(<RemotePane onConnect={handleConnect} />);

    const connectButton = screen.getByRole("button", { name: /New Connection.../i });
    fireEvent.click(connectButton);

    expect(handleConnect).toHaveBeenCalledTimes(1);
  });
});
