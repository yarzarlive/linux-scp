import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { AddressBar } from "../../src/components/FilePanel/AddressBar";
import { IpcClient } from "../../src/services/ipc";

vi.mock("../../src/services/ipc", () => ({
  IpcClient: {
    getSavedPaths: vi.fn().mockResolvedValue([]),
    saveSavedPaths: vi.fn().mockResolvedValue(undefined),
  },
}));

describe("AddressBar Component", () => {
  beforeEach(() => {
    if (typeof window !== "undefined" && window.localStorage) {
      window.localStorage.clear();
    }
    vi.clearAllMocks();
  });

  it("renders path input with current path", () => {
    render(<AddressBar path="/home/user/docs" onNavigate={vi.fn()} />);

    const input = screen.getByLabelText("Path location") as HTMLInputElement;
    expect(input.value).toBe("/home/user/docs");
  });

  it("submits new path on Enter key form submission", () => {
    const handleNavigate = vi.fn();
    render(<AddressBar path="/home/user" onNavigate={handleNavigate} />);

    const input = screen.getByLabelText("Path location");
    fireEvent.change(input, { target: { value: "/etc/nginx" } });
    fireEvent.submit(input);

    expect(handleNavigate).toHaveBeenCalledWith("/etc/nginx");
  });

  it("disables back button when canGoBack is false and enables when true", () => {
    const handleBack = vi.fn();
    const { rerender } = render(
      <AddressBar path="/home" onNavigate={vi.fn()} onBack={handleBack} canGoBack={false} />
    );

    const backBtn = screen.getByTitle("Back (Alt+Left)");
    expect(backBtn).toBeDisabled();

    rerender(<AddressBar path="/home" onNavigate={vi.fn()} onBack={handleBack} canGoBack={true} />);
    expect(backBtn).not.toBeDisabled();
    fireEvent.click(backBtn);
    expect(handleBack).toHaveBeenCalledTimes(1);
  });

  it("triggers parent, home, root, and refresh callbacks", () => {
    const handleUp = vi.fn();
    const handleHome = vi.fn();
    const handleRoot = vi.fn();
    const handleRefresh = vi.fn();
    const handleToggleHidden = vi.fn();

    render(
      <AddressBar
        path="/home/user"
        onNavigate={vi.fn()}
        onUp={handleUp}
        canGoUp={true}
        onHome={handleHome}
        onRoot={handleRoot}
        onRefresh={handleRefresh}
        showHidden={false}
        onToggleHidden={handleToggleHidden}
      />
    );

    fireEvent.click(screen.getByTitle("Parent Directory (..)"));
    expect(handleUp).toHaveBeenCalled();

    fireEvent.click(screen.getByTitle("Home Directory (~)"));
    expect(handleHome).toHaveBeenCalled();

    fireEvent.click(screen.getByTitle("Root Directory (/)"));
    expect(handleRoot).toHaveBeenCalled();

    fireEvent.click(screen.getByTitle("Refresh (F5)"));
    expect(handleRefresh).toHaveBeenCalled();

    fireEvent.click(screen.getByTitle("Show Hidden Files (Ctrl+H)"));
    expect(handleToggleHidden).toHaveBeenCalled();
  });

  it("saves current path and displays it in dropdown menu", async () => {
    render(<AddressBar path="/var/log" paneType="local" onNavigate={vi.fn()} />);

    // Click Save icon
    const saveBtn = screen.getByTitle("Save current path");
    fireEvent.click(saveBtn);

    expect(IpcClient.saveSavedPaths).toHaveBeenCalledWith("local", ["/var/log"]);

    // Open dropdown menu
    const dropdownBtn = screen.getByTitle("Saved paths");
    fireEvent.click(dropdownBtn);

    expect(screen.getByRole("menu")).toBeInTheDocument();
    expect(screen.getByText("/var/log")).toBeInTheDocument();
  });

  it("navigates to selected saved path and closes dropdown", async () => {
    const handleNavigate = vi.fn();
    (IpcClient.getSavedPaths as any).mockResolvedValueOnce(["/etc/nginx", "/var/www"]);

    render(<AddressBar path="/home" paneType="remote" onNavigate={handleNavigate} />);

    // Open dropdown
    const dropdownBtn = screen.getByTitle("Saved paths");
    fireEvent.click(dropdownBtn);

    // Wait for saved items
    await waitFor(() => {
      expect(screen.getByText("/var/www")).toBeInTheDocument();
    });

    // Click /var/www to navigate
    fireEvent.click(screen.getByText("/var/www"));

    expect(handleNavigate).toHaveBeenCalledWith("/var/www");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("deletes a saved path without closing the dropdown menu", async () => {
    (IpcClient.getSavedPaths as any).mockResolvedValueOnce(["/etc/nginx", "/var/www"]);

    render(<AddressBar path="/home" paneType="local" onNavigate={vi.fn()} />);

    // Open dropdown
    const dropdownBtn = screen.getByTitle("Saved paths");
    fireEvent.click(dropdownBtn);

    await waitFor(() => {
      expect(screen.getByText("/etc/nginx")).toBeInTheDocument();
      expect(screen.getByText("/var/www")).toBeInTheDocument();
    });

    // Delete /etc/nginx
    const deleteBtn = screen.getByTitle("Delete /etc/nginx");
    fireEvent.click(deleteBtn);

    expect(IpcClient.saveSavedPaths).toHaveBeenCalledWith("local", ["/var/www"]);

    // Dropdown should still remain open
    expect(screen.getByRole("menu")).toBeInTheDocument();
    expect(screen.queryByText("/etc/nginx")).not.toBeInTheDocument();
    expect(screen.getByText("/var/www")).toBeInTheDocument();
  });

  it("closes dropdown menu on Escape key press", async () => {
    render(<AddressBar path="/home" onNavigate={vi.fn()} />);

    const dropdownBtn = screen.getByTitle("Saved paths");
    fireEvent.click(dropdownBtn);
    expect(screen.getByRole("menu")).toBeInTheDocument();

    const input = screen.getByLabelText("Path location");
    fireEvent.keyDown(input, { key: "Escape" });

    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("isolates saved paths per site so different sites have different saved paths", async () => {
    (IpcClient.getSavedPaths as any).mockImplementation((scope: string) => {
      if (scope === "remote:prof-site-a") {
        return Promise.resolve(["/var/www/site-a", "/etc/nginx/sites-available"]);
      }
      if (scope === "remote:prof-site-b") {
        return Promise.resolve(["/home/ubuntu/site-b-app"]);
      }
      return Promise.resolve([]);
    });

    // 1. Render on Site A
    const { rerender } = render(
      <AddressBar path="/var/www/site-a" paneType="remote" siteId="prof-site-a" onNavigate={vi.fn()} />
    );

    expect(IpcClient.getSavedPaths).toHaveBeenCalledWith("remote:prof-site-a");

    const dropdownBtn = screen.getByTitle("Saved paths");
    fireEvent.click(dropdownBtn);

    await waitFor(() => {
      expect(screen.getByText("/var/www/site-a")).toBeInTheDocument();
      expect(screen.getByText("/etc/nginx/sites-available")).toBeInTheDocument();
      expect(screen.queryByText("/home/ubuntu/site-b-app")).not.toBeInTheDocument();
    });

    // 2. Switch to Site B
    rerender(
      <AddressBar path="/home/ubuntu/site-b-app" paneType="remote" siteId="prof-site-b" onNavigate={vi.fn()} />
    );

    expect(IpcClient.getSavedPaths).toHaveBeenCalledWith("remote:prof-site-b");

    // Reopen dropdown
    fireEvent.click(screen.getByTitle("Saved paths"));

    await waitFor(() => {
      expect(screen.getByText("/home/ubuntu/site-b-app")).toBeInTheDocument();
      expect(screen.queryByText("/var/www/site-a")).not.toBeInTheDocument();
      expect(screen.queryByText("/etc/nginx/sites-available")).not.toBeInTheDocument();
    });

    // 3. Save new path on Site B
    const saveBtn = screen.getByTitle("Save current path");
    fireEvent.click(saveBtn);

    expect(IpcClient.saveSavedPaths).toHaveBeenCalledWith("remote:prof-site-b", [
      "/home/ubuntu/site-b-app",
    ]);
  });
});
