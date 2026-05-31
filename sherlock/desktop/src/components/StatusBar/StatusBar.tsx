import type { FaceDetectProgress, RuntimeStatus } from "../../types";
import "./StatusBar.css";

declare const __APP_VERSION__: string;

type Props = {
  runtime: RuntimeStatus | null;
  isScanning: boolean;
  runningScansCount: number;
  selectedCount: number;
  faceProgress: FaceDetectProgress | null;
  onShowModelInfo?: () => void;
  onResetDb?: () => void;
};

export default function StatusBar({ runtime, isScanning, runningScansCount, selectedCount, faceProgress, onShowModelInfo, onResetDb }: Props) {
  return (
    <div className="statusbar">
      <span>
        {runtime?.provider === "groq" ? "Groq" : "Model"}: {runtime?.currentModel || "none"}
      </span>
      <span
        className={onShowModelInfo ? "statusbar-clickable" : undefined}
        onClick={onShowModelInfo}
        title="Click for model & hardware details"
        role={onShowModelInfo ? "button" : undefined}
        tabIndex={onShowModelInfo ? 0 : undefined}
        onKeyDown={onShowModelInfo ? (e) => { if (e.key === "Enter" || e.key === " ") onShowModelInfo(); } : undefined}
      >
        VRAM:{" "}
        {runtime?.vramUsedMib != null && runtime?.vramTotalMib != null
          ? `${runtime.vramUsedMib}/${runtime.vramTotalMib} MiB`
          : "n/a"}
      </span>
      {isScanning && (
        <span>Scanning: {runningScansCount} active job(s)</span>
      )}
      {faceProgress && (
        <span className="statusbar-face-progress">
          {faceProgress.phase === "downloading"
            ? "Downloading face models..."
            : faceProgress.phase === "loading"
              ? "Loading face models..."
              : `Faces: ${faceProgress.processed}/${faceProgress.total} (${faceProgress.facesFound} found)`}
        </span>
      )}
      {selectedCount > 0 && (
        <span>{selectedCount} selected</span>
      )}
      <span className="spacer" />
      {onResetDb && (
        <span
          className="statusbar-clickable statusbar-reset"
          onClick={onResetDb}
          title="Reset database — removes all data and caches"
          role="button"
          tabIndex={0}
          onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") onResetDb(); }}
        >
          Reset DB
        </span>
      )}
      <span className="statusbar-version">{__APP_VERSION__}</span>
    </div>
  );
}
