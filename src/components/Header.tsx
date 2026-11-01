import { copy } from "../copy";

export type UpdateState = "idle" | "checking" | "available" | "latest" | "error";

export interface HeaderProps {
  folderName: string;
  recording: boolean;
  updateState: UpdateState;
  updateVersion?: string;
  onToggleRecording(): void;
  onCheckUpdate(): void;
  onOpenUpdate(): void;
}

function updateText(state: UpdateState, version?: string): string {
  switch (state) {
    case "checking":
      return copy.updateChecking;
    case "available":
      return copy.updateAvailable(version ?? "");
    case "latest":
      return copy.updateLatest;
    case "error":
      return copy.updateError;
    case "idle":
      return "";
  }
}

export function Header({
  folderName,
  recording,
  updateState,
  updateVersion,
  onToggleRecording,
  onCheckUpdate,
  onOpenUpdate,
}: HeaderProps) {
  const text = updateText(updateState, updateVersion);
  return (
    <header className="mo-head">
      <div className="mono">
        <span className="dot" />
        {copy.eyebrow}
      </div>
      <h1>
        {copy.titleBefore} <em>{folderName}</em> {copy.titleAfter}
      </h1>
      <div className="head-actions">
        {text && (
          <span className="update-text" role="status">
            {text}
          </span>
        )}
        {updateState === "available" ? (
          <button type="button" className="mo-btn" onClick={onOpenUpdate}>
            {copy.updateOpen}
          </button>
        ) : (
          <button
            type="button"
            className="mo-btn sec"
            disabled={updateState === "checking"}
            onClick={onCheckUpdate}
          >
            {copy.checkUpdate}
          </button>
        )}
        <button
          type="button"
          className={`mo-btn sec${recording ? " on" : ""}`}
          aria-pressed={recording}
          aria-keyshortcuts="Control+Shift+R"
          title={copy.recordingHint}
          onClick={onToggleRecording}
        >
          {copy.recording}
        </button>
      </div>
    </header>
  );
}
