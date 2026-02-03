import { render, screen, fireEvent } from "@testing-library/react";
import FreeSpaceGuard from "../FreeSpaceGuard";

/**
 * Purpose: Verify the guard renders and toggles when minFree is set.
 *
 * Inputs: None.
 * Outputs: Asserts on guard text and toggle behavior.
 * Ties to: FreeSpaceGuard rendering and checkbox handler.
 * Side effects: Renders the component and fires UI events.
 * Why: Ensure the guard responds to user toggles.
 */
function assertGuardRendersAndToggles(): void {
  try {
    const onToggle = vi.fn();
    render(<FreeSpaceGuard minFree={1024} resumeOnSpace={false} onToggleResume={onToggle} />);
    expect(screen.getByText(/Free-space guard is active/i)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("checkbox"));
    expect(onToggle).toHaveBeenCalledWith(true);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[FreeSpaceGuard.test.tsx::assertGuardRendersAndToggles] ${reason}`);
  }
}

/**
 * Purpose: Verify byte labels render when minFree is provided.
 *
 * Inputs: None.
 * Outputs: Asserts on rendered byte label text.
 * Ties to: FreeSpaceGuard formatting logic.
 * Side effects: Renders the component.
 * Why: Keep byte formatting visible for operators.
 */
function assertBytesLabelShown(): void {
  try {
    render(<FreeSpaceGuard minFree={123} resumeOnSpace onToggleResume={() => {}} />);
    expect(screen.getByText(/123 bytes/i)).toBeInTheDocument();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[FreeSpaceGuard.test.tsx::assertBytesLabelShown] ${reason}`);
  }
}

describe("FreeSpaceGuard", () => {
  it("renders guard info when minFree is set", assertGuardRendersAndToggles);
  it("shows bytes label when provided", assertBytesLabelShown);
});
