import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AddressBar } from "../../src/components/FilePanel/AddressBar";

describe("AddressBar Component", () => {
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

    rerender(
      <AddressBar path="/home" onNavigate={vi.fn()} onBack={handleBack} canGoBack={true} />
    );
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
});
