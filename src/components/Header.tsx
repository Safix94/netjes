import { copy } from "../copy";

export interface HeaderProps {
  folderName: string;
  recording: boolean;
  onToggleRecording(): void;
}

export function Header({ folderName, recording, onToggleRecording }: HeaderProps) {
  return (
    <header className="mo-head">
      <div className="mono">
        <span className="dot" />
        {copy.eyebrow}
      </div>
      <h1>
        {copy.titleBefore} <em>{folderName}</em> {copy.titleAfter}
      </h1>
      <button
        type="button"
        className={`mo-btn sec small${recording ? " on" : ""}`}
        aria-pressed={recording}
        aria-keyshortcuts="Control+Shift+R"
        title={copy.recordingHint}
        onClick={onToggleRecording}
      >
        {copy.recording}
      </button>
    </header>
  );
}
