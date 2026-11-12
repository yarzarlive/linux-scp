import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { HostKeyPromptModal } from "../../src/components/SiteManager/HostKeyPromptModal";
import { ConnectOutcome } from "../../src/types/session";

describe("HostKeyPromptModal Component", () => {
  it("renders nothing when challenge is null", () => {
    const { container } = render(
      <HostKeyPromptModal challenge={null} onApproveUnknownKey={vi.fn()} onCancel={vi.fn()} />
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders unknown host key challenge with fingerprint and explicit trust action", () => {
    const onApprove = vi.fn();
    const onCancel = vi.fn();

    const unknownChallenge: Extract<ConnectOutcome, { outcome: "HostKeyUnknown" }> = {
      outcome: "HostKeyUnknown",
      host: "192.168.122.10",
      port: 22,
      algorithm: "ssh-ed25519",
      fingerprint_sha256: "SHA256:AbCdEfGhIjKlMnOpQrStUvWxYz1234567890",
      public_key_openssh: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITestKey",
    };

    render(
      <HostKeyPromptModal
        challenge={unknownChallenge}
        onApproveUnknownKey={onApprove}
        onCancel={onCancel}
      />
    );

    expect(screen.getByText("Verify Unknown SSH Host Key")).toBeInTheDocument();
    expect(screen.getAllByText("192.168.122.10:22").length).toBeGreaterThan(0);
    expect(screen.getByText("ssh-ed25519")).toBeInTheDocument();
    expect(screen.getByText("SHA256:AbCdEfGhIjKlMnOpQrStUvWxYz1234567890")).toBeInTheDocument();

    const approveBtn = screen.getByRole("button", { name: /Trust Host Key & Connect/i });
    fireEvent.click(approveBtn);
    expect(onApprove).toHaveBeenCalledWith(unknownChallenge);

    const rejectBtn = screen.getByRole("button", { name: /Reject & Disconnect/i });
    fireEvent.click(rejectBtn);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("renders changed host key MITM warning and blocks trust action", () => {
    const onApprove = vi.fn();
    const onCancel = vi.fn();

    const changedChallenge: Extract<ConnectOutcome, { outcome: "HostKeyChanged" }> = {
      outcome: "HostKeyChanged",
      host: "192.168.122.10",
      port: 22,
      expected_algorithm: "ssh-ed25519",
      expected_fingerprint: "SHA256:ExpectedKeyFingerprint",
      presented_algorithm: "ssh-rsa",
      presented_fingerprint: "SHA256:MismatchedKeyFingerprint",
    };

    render(
      <HostKeyPromptModal
        challenge={changedChallenge}
        onApproveUnknownKey={onApprove}
        onCancel={onCancel}
      />
    );

    expect(
      screen.getByText("SECURITY WARNING: Remote Host Identification Has Changed!")
    ).toBeInTheDocument();
    expect(screen.getByText("SHA256:ExpectedKeyFingerprint")).toBeInTheDocument();
    expect(screen.getByText("SHA256:MismatchedKeyFingerprint")).toBeInTheDocument();

    // Must never allow trusting a changed host key automatically
    expect(
      screen.queryByRole("button", { name: /Trust Host Key & Connect/i })
    ).not.toBeInTheDocument();

    const abortBtn = screen.getByRole("button", { name: /Abort & Close/i });
    fireEvent.click(abortBtn);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});
