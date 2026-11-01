import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useEffect } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../App";
import type { AnimFile, FlyCanvasProps } from "../components/FlyCanvas";
import { copy } from "../copy";
import * as api from "../lib/api";
import { CATEGORIES, file } from "./fixtures";

vi.mock("../lib/api", () => ({
  LOG_SAVE_FAILED_ID: 4294967295,
  getCategories: vi.fn(),
  getDownloadsDir: vi.fn(),
  scan: vi.fn(),
  checkUndo: vi.fn(),
  undoOrganize: vi.fn(),
  disorganize: vi.fn(),
  beginOrganize: vi.fn(),
  moveFiles: vi.fn(),
  getThumbnails: vi.fn(),
  listFolderFiles: vi.fn(),
  chooseFolder: vi.fn(),
  confirm: vi.fn(),
  revealInExplorer: vi.fn(),
  openPath: vi.fn(),
  openUrl: vi.fn(),
  checkUpdate: vi.fn(),
  DOWNLOAD_PAGE: "https://www.agentiek.be/netjes",
  thumbUrl: (p: string) => p,
}));

// Het canvas zelf wordt in FlyCanvas.test.tsx getest; hier landt elke tegel zodra zijn uitkomst er is.
vi.mock("../components/FlyCanvas", () => ({
  FlyCanvas: (props: FlyCanvasProps) => {
    useEffect(() => {
      if (props.phase !== "sorting") return;
      const done = new Set<number>();
      const timer = setInterval(() => {
        for (const f of props.files as AnimFile[]) {
          if (f.launchAt === undefined || done.has(f.id)) continue;
          const o = props.outcomes.get(f.id);
          if (!o) continue;
          done.add(f.id);
          if (o.ok) props.onLand(f.id);
          else props.onReturn(f.id);
        }
      }, 5);
      return () => clearInterval(timer);
    }, [props]);
    return <canvas data-testid="fly" />;
  },
}));

const ROOT = "C:\\Users\\Test\\Downloads";

const scanned = (root: string, ...categories: api.Category[]): api.ScanResult => ({
  root,
  files: categories.map(file),
  existing_category_folders: ["fotos"],
  skipped: 0,
});

const button = (name: string | RegExp) => screen.getByRole("button", { name });

/** Nep-API voor het verplaatsen: alles lukt, behalve de bestanden waarvan de naam in `failing` staat. */
function mockMoves(failing: string[] = []) {
  let lastScan: api.ScanResult | null = null;
  vi.mocked(api.scan).mockImplementation(async (root) => {
    lastScan = scanned(root, "fotos", "fotos", "videos", "duplicaten");
    return lastScan;
  });
  vi.mocked(api.beginOrganize).mockImplementation(async () => ({
    moves: (lastScan?.files ?? []).map((f) => ({ id: f.id, src: f.path, dest: `${ROOT}\\map\\${f.name}`, category: f.category })),
    folders_created: [],
  }));
  vi.mocked(api.moveFiles).mockImplementation(async (_root, ids) =>
    ids.map((id) => {
      const f = lastScan!.files.find((x) => x.id === id)!;
      return failing.includes(f.name)
        ? { id, ok: false, dest: null, error: "Het bestand is in gebruik." }
        : { id, ok: true, dest: `${ROOT}\\map\\${f.name}`, error: null };
    }),
  );
  return () => lastScan!;
}

beforeEach(() => {
  vi.mocked(api.getCategories).mockResolvedValue(CATEGORIES);
  vi.mocked(api.getDownloadsDir).mockResolvedValue(ROOT);
  vi.mocked(api.scan).mockImplementation(async (root) => scanned(root, "fotos", "fotos", "videos", "duplicaten"));
  vi.mocked(api.checkUndo).mockResolvedValue(null);
  vi.mocked(api.revealInExplorer).mockResolvedValue();
  vi.mocked(api.getThumbnails).mockImplementation(async (paths) => paths.map(() => null));
  vi.mocked(api.listFolderFiles).mockResolvedValue([]);
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("App", () => {
  it("scans the Downloads folder on start and shows the real numbers", async () => {
    render(<App />);
    expect(await screen.findByText(copy.statusReady(4))).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Mijn Downloads opruimen");
    expect(screen.getByRole("img", { name: "4" })).toBeInTheDocument(); // bestanden
    expect(screen.getByRole("img", { name: "2" })).toBeInTheDocument(); // foto's
    expect(button(copy.organize(4))).toBeEnabled();
    expect(button(copy.undo)).toBeDisabled();
    expect(screen.getByTestId("fly")).toBeInTheDocument();
  });

  it("calls an empty folder tidy and does not offer to organize it", async () => {
    vi.mocked(api.scan).mockImplementation(async (root) => scanned(root));
    render(<App />);
    expect(await screen.findByText(copy.statusTidy)).toBeInTheDocument();
    expect(button(copy.organize(0))).toBeDisabled();
  });

  it("shows a refused folder in the status and keeps Map kiezen available", async () => {
    vi.mocked(api.scan).mockRejectedValue("Die map ruimt Netjes niet op. Kies een gewone map zoals Downloads.");
    render(<App />);
    expect(await screen.findByText(copy.forbiddenRoot)).toBeInTheDocument();
    expect(button(copy.organize(0))).toBeDisabled();
    expect(button(copy.chooseFolder)).toBeEnabled();
  });

  it("keeps the previous folder when a newly chosen folder is refused", async () => {
    render(<App />);
    await screen.findByText(copy.statusReady(4));
    vi.mocked(api.chooseFolder).mockResolvedValue("C:\\Windows");
    vi.mocked(api.scan).mockRejectedValue("Die map ruimt Netjes niet op. Kies een gewone map zoals Downloads.");
    fireEvent.click(button(copy.chooseFolder));
    expect(await screen.findByText(copy.forbiddenRoot)).toBeInTheDocument();
    expect(api.scan).toHaveBeenLastCalledWith("C:\\Windows");
    expect(screen.getByTitle(ROOT)).toBeInTheDocument();
    expect(button(copy.organize(4))).toBeEnabled();
  });

  it("opens the folder viewer for an existing category folder and reveals it from there", async () => {
    vi.mocked(api.listFolderFiles).mockResolvedValue([file("fotos")]);
    render(<App />);
    await screen.findByText(copy.statusReady(4));
    fireEvent.click(document.querySelector('[data-cat="audio"]')!);
    expect(api.listFolderFiles).not.toHaveBeenCalled();
    fireEvent.click(document.querySelector('[data-cat="fotos"]')!);
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    expect(api.listFolderFiles).toHaveBeenCalledWith(ROOT, "fotos");
    fireEvent.click(button(copy.reveal));
    expect(api.revealInExplorer).toHaveBeenCalledWith(`${ROOT}\\Foto's`);
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("asks before undoing, then restores and scans again", async () => {
    vi.mocked(api.checkUndo).mockResolvedValue({ count: 5, created_at_ms: 1 });
    vi.mocked(api.confirm).mockResolvedValue(true);
    vi.mocked(api.undoOrganize).mockResolvedValue({ restored: 5, errors: [] });
    render(<App />);
    await waitFor(() => expect(button(copy.undo)).toBeEnabled());
    fireEvent.click(button(copy.undo));
    await waitFor(() => expect(api.undoOrganize).toHaveBeenCalledWith(ROOT));
    expect(api.confirm).toHaveBeenCalledWith(copy.confirmUndo(5), copy.appName);
    await waitFor(() => expect(api.scan).toHaveBeenCalledTimes(2));
  });

  it("shows the generic Dutch message instead of an English plugin error", async () => {
    vi.mocked(api.listFolderFiles).mockRejectedValue(new Error("failed to open path: access denied"));
    render(<App />);
    await screen.findByText(copy.statusReady(4));
    fireEvent.click(document.querySelector('[data-cat="fotos"]')!);
    expect(await screen.findByText(copy.genericError)).toBeInTheDocument();
    expect(screen.queryByText(/access denied/)).not.toBeInTheDocument();
  });

  it("clears an error on the next action so the normal status returns", async () => {
    render(<App />);
    await screen.findByText(copy.statusReady(4));
    vi.mocked(api.chooseFolder).mockResolvedValue("C:\\Windows");
    vi.mocked(api.scan).mockRejectedValueOnce(copy.forbiddenRoot);
    fireEvent.click(button(copy.chooseFolder));
    expect(await screen.findByText(copy.forbiddenRoot)).toBeInTheDocument();
    fireEvent.click(document.querySelector('[data-cat="audio"]')!);
    expect(await screen.findByText(copy.statusReady(4))).toBeInTheDocument();
    expect(screen.queryByText(copy.forbiddenRoot)).not.toBeInTheDocument();
  });

  it("toggles recording mode with Ctrl+Shift+R and with the Opname button", async () => {
    render(<App />);
    await screen.findByText(copy.statusReady(4));
    expect(screen.queryByText(copy.disorganize)).not.toBeInTheDocument();
    fireEvent.keyDown(window, { key: "R", ctrlKey: true, shiftKey: true });
    expect(await screen.findByText(copy.disorganize)).toBeInTheDocument();
    expect(screen.getByText(copy.resort)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: copy.recording }));
    expect(screen.queryByText(copy.disorganize)).not.toBeInTheDocument();
  });

  it("organizes: lands the tiles, counts every move, lists the failure and enables undo", async () => {
    const last = mockMoves();
    render(<App />);
    await screen.findByText(copy.statusReady(4));
    const videoName = last().files.find((f) => f.category === "videos")!.name;
    vi.mocked(api.moveFiles).mockImplementation(async (_root, ids) =>
      ids.map((id) => {
        const f = last().files.find((x) => x.id === id)!;
        return f.name === videoName
          ? { id, ok: false, dest: null, error: "Het bestand is in gebruik." }
          : { id, ok: true, dest: `${ROOT}\\map\\${f.name}`, error: null };
      }),
    );
    vi.mocked(api.checkUndo).mockResolvedValue({ count: 3, created_at_ms: 1 });
    fireEvent.click(button(copy.organize(4)));
    expect(await screen.findByText(copy.statusSorting)).toBeInTheDocument();
    expect(await screen.findByText(copy.statusDone)).toBeInTheDocument();
    expect(api.beginOrganize).toHaveBeenCalledWith(ROOT);
    expect(api.moveFiles).toHaveBeenCalled();
    expect(screen.getByRole("img", { name: "3" })).toBeInTheDocument(); // bestanden verplaatst
    expect(screen.getByText(copy.errorsTitle(1))).toBeInTheDocument();
    expect(screen.getByText(videoName)).toBeInTheDocument();
    expect(screen.getByText("Het bestand is in gebruik.")).toBeInTheDocument();
    expect(document.querySelector('[data-cat="fotos"]')).toHaveClass("active");
    expect(document.querySelector('[data-cat="videos"]')).not.toHaveClass("active");
    expect(document.querySelector('[data-cat="fotos"] .ccount')).toHaveTextContent("2");
    await waitFor(() => expect(button(copy.undo)).toBeEnabled());
    expect(screen.getByText(copy.capTidy)).toBeInTheDocument();
  });

  it("reports a failed undo log without counting it as a file", async () => {
    const last = mockMoves();
    render(<App />);
    await screen.findByText(copy.statusReady(4));
    vi.mocked(api.moveFiles).mockImplementation(async (_root, ids) => [
      { id: api.LOG_SAVE_FAILED_ID, ok: false, dest: null, error: "schijf vol" },
      ...ids.map((id) => ({ id, ok: true, dest: `${ROOT}\\map\\${last().files.find((x) => x.id === id)!.name}`, error: null })),
    ]);
    fireEvent.click(button(copy.organize(4)));
    expect(await screen.findByText(copy.statusDone)).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "4" })).toBeInTheDocument();
    expect(screen.getByText(copy.logNotSaved)).toBeInTheDocument();
  });

  it("checks for updates on request and offers the download page when a newer version exists", async () => {
    vi.mocked(api.checkUpdate).mockResolvedValue({ state: "available", version: "1.1.0" });
    vi.mocked(api.openUrl).mockResolvedValue();
    render(<App />);
    await screen.findByText(copy.statusReady(4));
    fireEvent.click(button(copy.checkUpdate));
    expect(await screen.findByText(copy.updateAvailable("1.1.0"))).toBeInTheDocument();
    fireEvent.click(button(copy.updateOpen));
    expect(api.openUrl).toHaveBeenCalledWith("https://www.agentiek.be/netjes");

    vi.mocked(api.checkUpdate).mockResolvedValue({ state: "error", version: null });
    fireEvent.click(button(copy.updateOpen));
    cleanup();
    render(<App />);
    await screen.findByText(copy.statusReady(4));
    fireEvent.click(button(copy.checkUpdate));
    expect(await screen.findByText(copy.updateError)).toBeInTheDocument();
  });

  it("in recording mode, Rommelig maken asks first and Opnieuw sorteren runs straight through", async () => {
    mockMoves();
    vi.mocked(api.confirm).mockResolvedValue(true);
    vi.mocked(api.disorganize).mockResolvedValue({ restored: 4, errors: [] });
    render(<App />);
    await screen.findByText(copy.statusReady(4));
    fireEvent.keyDown(window, { key: "R", ctrlKey: true, shiftKey: true });
    fireEvent.click(await screen.findByText(copy.disorganize));
    await waitFor(() => expect(api.disorganize).toHaveBeenCalledWith(ROOT));
    expect(api.confirm).toHaveBeenCalledWith(copy.confirmDisorganize, copy.appName);
    await waitFor(() => expect(api.scan).toHaveBeenCalledTimes(2));
    expect(await screen.findByText(copy.statusReady(4))).toBeInTheDocument();

    vi.mocked(api.confirm).mockClear();
    fireEvent.click(screen.getByText(copy.resort));
    await waitFor(() => expect(api.disorganize).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(api.beginOrganize).toHaveBeenCalledWith(ROOT));
    expect(api.confirm).not.toHaveBeenCalled();
    expect(await screen.findByText(copy.statusDone)).toBeInTheDocument();
  });
});
