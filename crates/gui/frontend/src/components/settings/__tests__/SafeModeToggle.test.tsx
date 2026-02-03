import { render, screen, fireEvent } from "@testing-library/react";
import SafeModeToggle from "../SafeModeToggle";

/**
 * Purpose: Verify the toggle wiring updates safe mode state.
 *
 * Inputs: None.
 * Outputs: Asserts that the change handler receives the toggled value.
 * Ties to: SafeModeToggle checkbox interaction.
 * Side effects: Renders the component and fires UI events.
 * Why: Ensure safe mode can be enabled from the UI.
 */
function assertToggleUpdatesValue(): void {
  try {
    const onChange = vi.fn();
    render(<SafeModeToggle value={false} onChange={onChange} />);
    const checkbox = screen.getByRole("checkbox");
    fireEvent.click(checkbox);
    expect(onChange).toHaveBeenCalledWith(true);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[SafeModeToggle.test.tsx::assertToggleUpdatesValue] ${reason}`);
  }
}

/**
 * Purpose: Verify helper text renders for safe mode.
 *
 * Inputs: None.
 * Outputs: Asserts helper text visibility.
 * Ties to: SafeModeToggle rendering.
 * Side effects: Renders the component.
 * Why: Keep the safe mode guidance visible to operators.
 */
function assertHelperTextVisible(): void {
  try {
    render(<SafeModeToggle value={true} onChange={() => {}} />);
    expect(screen.getByText(/Safe mode/)).toBeInTheDocument();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[SafeModeToggle.test.tsx::assertHelperTextVisible] ${reason}`);
  }
}

describe("SafeModeToggle", () => {
  it("renders label and toggles", assertToggleUpdatesValue);
  it("shows helper text", assertHelperTextVisible);
});
