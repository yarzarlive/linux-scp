import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PropertiesDialog } from "../../src/components/PropertiesDialog/PropertiesDialog";
import { FileEntry } from "../../src/types/filesystem";

function makeEntry(overrides?: Partial<FileEntry>): FileEntry {
  return {
    name: "app.sh",
    path: "/remote/app.sh",
    file_type: { type: "Regular" },
    metadata: {
      size: 512,
      modified_at_secs: 1700000000,
      permissions: {
        raw_mode: 0o100644,
        user_read: true,
        user_write: true,
        user_execute: false,
        group_read: true,
        group_write: false,
        group_execute: false,
        other_read: true,
        other_write: false,
        other_execute: false,
        set_uid: false,
        set_gid: false,
        sticky: false,
      },
      ownership: {
        uid: 1000,
        gid: 1000,
      },
    },
    is_hidden: false,
    ...overrides,
  };
}

describe("PropertiesDialog Component", () => {
  it("synchronizes matrix checkboxes, special bits, octal input, and symbolic preview", async () => {
    const onApplyPermissions = vi.fn().mockResolvedValue(undefined);
    render(
      <PropertiesDialog
        isOpen={true}
        entries={[makeEntry()]}
        canSetPermissions={true}
        canSetOwnership={true}
        onApplyPermissions={onApplyPermissions}
        onApplyOwnership={vi.fn()}
        onClose={vi.fn()}
      />
    );

    const octalInput = screen.getByLabelText("Octal:") as HTMLInputElement;
    expect(octalInput.value).toBe("0644");
    expect(screen.getByTestId("symbolic-preview")).toHaveTextContent("rw-r--r--");

    // Toggle Owner Execute and SUID
    fireEvent.click(screen.getByLabelText("Owner Execute"));
    fireEvent.click(screen.getByLabelText("SUID"));

    expect(octalInput.value).toBe("4744");
    expect(screen.getByTestId("symbolic-preview")).toHaveTextContent("rwsr--r--");

    // Validate invalid octal
    fireEvent.change(octalInput, { target: { value: "888" } });
    expect(screen.getByText(/Enter 3 or 4 octal digits/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Apply Permissions" })).toBeDisabled();

    // Valid octal
    fireEvent.change(octalInput, { target: { value: "755" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply Permissions" }));

    await waitFor(() => {
      expect(onApplyPermissions).toHaveBeenCalledWith(["/remote/app.sh"], "0755", false);
    });
  });

  it("requires explicit confirmation before applying recursive permissions to directories", async () => {
    const onApplyPermissions = vi.fn().mockResolvedValue(undefined);
    render(
      <PropertiesDialog
        isOpen={true}
        entries={[
          makeEntry({
            name: "www",
            path: "/remote/www",
            file_type: { type: "Directory" },
          }),
        ]}
        canSetPermissions={true}
        canSetOwnership={true}
        onApplyPermissions={onApplyPermissions}
        onApplyOwnership={vi.fn()}
        onClose={vi.fn()}
      />
    );

    fireEvent.click(
      screen.getByLabelText("Apply permissions recursively to subdirectories and files")
    );
    fireEvent.click(screen.getByRole("button", { name: "Apply Permissions" }));

    expect(onApplyPermissions).not.toHaveBeenCalled();
    expect(screen.getByText(/Confirm Recursive Permission Change/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Confirm Recursive Apply" }));

    await waitFor(() => {
      expect(onApplyPermissions).toHaveBeenCalledWith(["/remote/www"], "0644", true);
    });
  });

  it("displays numeric UID/GID, optional getent resolution, and server rejection on ownership change", async () => {
    const onResolveIdentity = vi.fn().mockResolvedValue({
      uid: 1000,
      gid: 1000,
      user_name: "ubuntu",
      group_name: "ubuntu",
    });
    const onApplyOwnership = vi.fn().mockRejectedValue({
      kind: "PermissionDenied",
      message: "Server rejected ownership change on '/remote/app.sh': Permission denied",
    });

    render(
      <PropertiesDialog
        isOpen={true}
        entries={[makeEntry()]}
        canSetPermissions={true}
        canSetOwnership={true}
        onResolveIdentity={onResolveIdentity}
        onApplyPermissions={vi.fn()}
        onApplyOwnership={onApplyOwnership}
        onClose={vi.fn()}
      />
    );

    expect(screen.getByText("UID 1000")).toBeInTheDocument();
    expect(screen.getByText("GID 1000")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Resolve Names (getent)" }));
    await waitFor(() => {
      expect(screen.getAllByText("ubuntu (1000)").length).toBe(2);
    });

    fireEvent.change(screen.getByLabelText("Owner (UID or name)"), { target: { value: "0" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply Ownership" }));

    await waitFor(() => {
      expect(
        screen.getByText("Server rejected ownership change on '/remote/app.sh': Permission denied")
      ).toBeInTheDocument();
    });
  });

  it("shows clear unavailable notices when capabilities are disabled or multi-selection is active", () => {
    render(
      <PropertiesDialog
        isOpen={true}
        entries={[
          makeEntry({ name: "a.txt", path: "/remote/a.txt" }),
          makeEntry({ name: "b.txt", path: "/remote/b.txt" }),
        ]}
        canSetPermissions={false}
        canSetOwnership={false}
        onApplyPermissions={vi.fn()}
        onApplyOwnership={vi.fn()}
        onClose={vi.fn()}
      />
    );

    expect(
      screen.getByText("Permission changes are not supported by this session protocol.")
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "Ownership editing is unavailable for multi-selection. Select a single item to view or edit owner/group."
      )
    ).toBeInTheDocument();
  });
});
