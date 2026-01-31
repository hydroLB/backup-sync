import { render, screen, fireEvent } from "@testing-library/react";
import React from "react";
import AuthLockModal from "../AuthLockModal";

/**
 * Purpose: Ensure the unlock handler fires when the button is clicked.
 *
 * Inputs: None.
 * Outputs: Asserts that the unlock handler was called.
 * Ties to: Auth lock modal button wiring.
 * Side effects: Renders and interacts with the modal component.
 * Why: Confirm the unlock button triggers the expected callback.
 */
function assertUnlockHandlerInvoked(): void {
  try {
    const onUnlock = vi.fn();
    render(
      <AuthLockModal
        visible
        passcode=""
        unlockSeconds={900}
        onChange={() => {}}
        onUnlock={onUnlock}
        onClose={() => {}}
      />
    );
    const button = screen.getByRole("button", { name: /Unlock/i });
    fireEvent.click(button);
    expect(onUnlock).toHaveBeenCalled();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[AuthLockModal.test.tsx::assertUnlockHandlerInvoked] ${reason}`);
  }
}

/**
 * Purpose: Ensure the modal renders the guidance copy when visible.
 *
 * Inputs: None.
 * Outputs: Asserts on guidance text presence.
 * Ties to: Auth lock modal rendering.
 * Side effects: Renders the modal component.
 * Why: Keep unlock guidance visible to operators.
 */
function assertGuidanceCopyVisible(): void {
  try {
    render(
      <AuthLockModal
        visible
        passcode=""
        unlockSeconds={900}
        onChange={() => {}}
        onUnlock={() => {}}
        onClose={() => {}}
      />
    );
    expect(screen.getByText(/Unlock to edit/i)).toBeInTheDocument();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[AuthLockModal.test.tsx::assertGuidanceCopyVisible] ${reason}`);
  }
}

describe("AuthLockModal", () => {
  it("invokes unlock handler", assertUnlockHandlerInvoked);
  it("shows guidance copy when locked", assertGuidanceCopyVisible);
});
