import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { RemoteTabBar } from "../../src/components/FilePanel/RemoteTabBar";
import { createDefaultTabState } from "../../src/types/tabs";

describe("RemoteTabBar Component", () => {
  it("renders tabs and highlights the active tab", () => {
    const tab1 = createDefaultTabState("tab-1", "Production Web");
    const tab2 = createDefaultTabState("tab-2", "Staging DB");

    render(
      <RemoteTabBar
        tabs={[tab1, tab2]}
        activeTabId="tab-1"
        onSelectTab={vi.fn()}
        onCloseTab={vi.fn()}
        onAddTab={vi.fn()}
      />
    );

    const tabs = screen.getAllByRole("tab");
    expect(tabs).toHaveLength(2);
    expect(tabs[0]).toHaveAttribute("aria-selected", "true");
    expect(tabs[1]).toHaveAttribute("aria-selected", "false");
    expect(screen.getByText("Production Web")).toBeInTheDocument();
    expect(screen.getByText("Staging DB")).toBeInTheDocument();
  });

  it("calls onSelectTab when an inactive tab is clicked", () => {
    const tab1 = createDefaultTabState("tab-1", "Production Web");
    const tab2 = createDefaultTabState("tab-2", "Staging DB");
    const onSelectTab = vi.fn();

    render(
      <RemoteTabBar
        tabs={[tab1, tab2]}
        activeTabId="tab-1"
        onSelectTab={onSelectTab}
        onCloseTab={vi.fn()}
        onAddTab={vi.fn()}
      />
    );

    fireEvent.click(screen.getByText("Staging DB"));
    expect(onSelectTab).toHaveBeenCalledWith("tab-2");
  });

  it("calls onSelectTab on Enter or Space key down", () => {
    const tab1 = createDefaultTabState("tab-1", "Production Web");
    const tab2 = createDefaultTabState("tab-2", "Staging DB");
    const onSelectTab = vi.fn();

    render(
      <RemoteTabBar
        tabs={[tab1, tab2]}
        activeTabId="tab-1"
        onSelectTab={onSelectTab}
        onCloseTab={vi.fn()}
        onAddTab={vi.fn()}
      />
    );

    const tabs = screen.getAllByRole("tab");
    fireEvent.keyDown(tabs[1], { key: "Enter" });
    expect(onSelectTab).toHaveBeenCalledWith("tab-2");

    fireEvent.keyDown(tabs[1], { key: " " });
    expect(onSelectTab).toHaveBeenCalledWith("tab-2");
  });

  it("calls onAddTab when the plus button is clicked", () => {
    const tab1 = createDefaultTabState("tab-1", "Remote 1");
    const onAddTab = vi.fn();

    render(
      <RemoteTabBar
        tabs={[tab1]}
        activeTabId="tab-1"
        onSelectTab={vi.fn()}
        onCloseTab={vi.fn()}
        onAddTab={onAddTab}
      />
    );

    const addBtn = screen.getByLabelText("New connection tab");
    fireEvent.click(addBtn);
    expect(onAddTab).toHaveBeenCalledTimes(1);
  });

  it("calls onCloseTab when close button is clicked without triggering onSelectTab", () => {
    const tab1 = createDefaultTabState("tab-1", "Tab 1");
    const tab2 = createDefaultTabState("tab-2", "Tab 2");
    const onSelectTab = vi.fn();
    const onCloseTab = vi.fn();

    render(
      <RemoteTabBar
        tabs={[tab1, tab2]}
        activeTabId="tab-1"
        onSelectTab={onSelectTab}
        onCloseTab={onCloseTab}
        onAddTab={vi.fn()}
      />
    );

    const closeBtn = screen.getByLabelText("Close tab Tab 2");
    fireEvent.click(closeBtn);
    expect(onCloseTab).toHaveBeenCalledWith("tab-2");
    expect(onSelectTab).not.toHaveBeenCalled();
  });

  it("does not render close button when only one tab exists", () => {
    const tab1 = createDefaultTabState("tab-1", "Solo Tab");

    render(
      <RemoteTabBar
        tabs={[tab1]}
        activeTabId="tab-1"
        onSelectTab={vi.fn()}
        onCloseTab={vi.fn()}
        onAddTab={vi.fn()}
      />
    );

    expect(screen.queryByLabelText("Close tab Solo Tab")).not.toBeInTheDocument();
  });
});
