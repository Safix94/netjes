export type Phase = "idle" | "scanning" | "ready" | "tidy" | "sorting" | "done" | "undoing" | "error";

export function StatusPill({ phase, text }: { phase: Phase; text: string }) {
  return (
    <div className="mo-status">
      <div className={`mo-pill ${phase}`} role="status">
        <span className="pdot" />
        <span>{text}</span>
      </div>
    </div>
  );
}
