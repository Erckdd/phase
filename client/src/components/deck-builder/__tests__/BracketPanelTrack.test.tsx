import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";

import { BracketPanelTrack } from "../BracketPanelTrack";

afterEach(cleanup);

function segment(container: HTMLElement, n: number): HTMLElement {
  const el = container.querySelector<HTMLElement>(`[data-segment="${n}"]`);
  if (!el) throw new Error(`segment ${n} missing`);
  return el;
}

describe("BracketPanelTrack", () => {
  it("fills B2 up to the estimate and marks it", () => {
    const { container } = render(<BracketPanelTrack tier="optimized" declared={null} />);
    expect([1, 2, 3, 4, 5].map((n) => segment(container, n).dataset.filled)).toEqual([
      "false",
      "true",
      "true",
      "true",
      "false",
    ]);
    expect(screen.getByText("B4 ▲")).toBeInTheDocument();
  });

  it("puts the You marker over the declared bracket", () => {
    const { container } = render(<BracketPanelTrack tier="upgraded" declared={3} />);
    expect(within(segment(container, 3)).getByText("You ▼")).toBeInTheDocument();
    expect(screen.getAllByText("You ▼")).toHaveLength(1);
  });

  it("is hidden from screen readers", () => {
    const { container } = render(<BracketPanelTrack tier="core" declared={null} />);
    expect(container.firstElementChild).toHaveAttribute("aria-hidden", "true");
  });
});
