import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Controls } from "../components/Controls";
import { ErrorList } from "../components/ErrorList";
import { copy } from "../copy";

describe("ErrorList", () => {
  it("renders nothing when there are no errors", () => {
    const { container } = render(<ErrorList errors={[]} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("sits in the controls column under the path line and lists each file", () => {
    const noop = () => {};
    const { container } = render(
      <Controls
        phase="done"
        fileCount={3}
        recording={false}
        canUndo
        onOrganize={noop}
        onUndo={noop}
        onDisorganize={noop}
        onResort={noop}
        onChooseFolder={noop}
        rootPath="C:\Users\Test\Downloads"
      >
        <ErrorList errors={[{ name: "a.pdf", error: "In gebruik" }]} />
      </Controls>,
    );
    const alert = screen.getByRole("alert");
    expect(container.querySelector(".mo-ctrl")).toContainElement(alert);
    expect(alert).toHaveTextContent(copy.errorsTitle(1));
    expect(alert).toHaveTextContent("a.pdf In gebruik");
    expect(container.querySelector(".path")!.compareDocumentPosition(alert)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
  });
});
