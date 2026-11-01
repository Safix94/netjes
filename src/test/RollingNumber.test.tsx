import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { RollingNumber } from "../components/RollingNumber";

describe("RollingNumber", () => {
  it("renders one strip per digit and moves it", () => {
    render(<RollingNumber value={431} />);
    expect(screen.getAllByTestId("digit")).toHaveLength(3);
    expect(screen.getAllByTestId("digit")[0].firstElementChild).toHaveStyle({
      transform: "translateY(-4em)",
    });
  });
});
