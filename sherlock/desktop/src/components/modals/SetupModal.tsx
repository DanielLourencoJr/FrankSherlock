import { openUrl } from "@tauri-apps/plugin-opener";
import type { SetupStatus } from "../../types";
import ModalOverlay from "./ModalOverlay";
import "./shared-modal.css";
import "./SetupModal.css";

type Props = {
  setup: SetupStatus;
  onRecheck: () => void;
  onDownload: () => void;
  onSetupOcr: () => void;
  onClose?: () => void;
};

function ExternalLink({ url, children }: { url: string; children: React.ReactNode }) {
  return (
    <a
      href="#"
      className="setup-link"
      onClick={(e) => { e.preventDefault(); openUrl(url); }}
    >
      {children}
    </a>
  );
}

export default function SetupModal({ setup, onRecheck, onDownload, onSetupOcr, onClose }: Props) {
  const isGroq = setup.provider === "groq";

  const ocrStatusText = isGroq
    ? "Groq handles OCR natively"
    : setup.suryaVenvOk
      ? `Ready${setup.pythonVersion ? ` (Python ${setup.pythonVersion})` : ""}`
      : setup.systemPythonFound
        ? "Python found, needs setup"
        : setup.pythonAvailable
          ? "Python found, venv issue"
          : null;

  const canSetupOcr =
    !isGroq &&
    setup.systemPythonFound &&
    !setup.suryaVenvOk &&
    setup.venvProvision.status !== "running";

  return (
    <ModalOverlay onEscape={onClose}>
      <div className="modal-base setup-modal" onClick={(e) => e.stopPropagation()}>
        <h2>First-Time Setup</h2>
        <p>
          {isGroq
            ? "Sherlock uses the Groq API for classification and OCR. Configure your API key below."
            : "Sherlock needs local Ollama service and required model(s) before scanning."}
        </p>
        <div className="setup-status-grid">
          {isGroq ? (
            <>
              <div>
                <strong>Provider</strong>
                <p>Groq (Llama 4 Scout)</p>
              </div>
              <div>
                <strong>API Key</strong>
                <p>
                  {setup.groqConfigured
                    ? "Configured"
                    : <>Not configured — <ExternalLink url="https://console.groq.com/keys">get a key</ExternalLink></>}
                </p>
              </div>
            </>
          ) : (
            <>
              <div>
                <strong>Ollama</strong>
                <p>{setup.ollamaAvailable ? "Running" : <>Not detected — <ExternalLink url="https://ollama.com/download">install</ExternalLink></>}</p>
              </div>
              <div>
                <strong>Model ({setup.modelTier})</strong>
                <p title={setup.modelSelectionReason}>{setup.recommendedModel}</p>
              </div>
              <div>
                <strong>Missing</strong>
                <p>{setup.missingModels.length ? setup.missingModels.join(", ") : "None"}</p>
              </div>
            </>
          )}
          <div>
            <strong>OCR</strong>
            <p>{isGroq ? "Groq (built-in)" : ocrStatusText ?? <>Not available — <ExternalLink url="https://www.python.org/downloads/">install Python</ExternalLink></>}</p>
          </div>
          <div>
            <strong>Video (ffmpeg)</strong>
            <p>{setup.ffmpegAvailable ? "Available" : "Not found"}</p>
          </div>
        </div>
        <ul className="setup-instructions">
          {setup.instructions.map((instruction) => (
            <li key={instruction}>{instruction}</li>
          ))}
        </ul>
        {!isGroq && (
          <>
            <div className="progress-wrap">
              <progress value={setup.download.progressPct} max={100} />
              <span>{setup.download.progressPct.toFixed(1)}%</span>
            </div>
            <p className="setup-download-text">{setup.download.message}</p>
          </>
        )}
        {setup.venvProvision.status !== "idle" && !isGroq && (
          <>
            <div className="progress-wrap">
              <progress value={setup.venvProvision.status === "completed" ? 100 : setup.venvProvision.progressPct} max={100} />
              <span>{(setup.venvProvision.status === "completed" ? 100 : setup.venvProvision.progressPct).toFixed(1)}%</span>
            </div>
            <p className="setup-download-text">{setup.venvProvision.message}</p>
          </>
        )}
        <div className="modal-actions">
          <button type="button" onClick={onRecheck}>Recheck</button>
          {!isGroq && (
            <button
              type="button"
              onClick={onDownload}
              disabled={
                !setup.ollamaAvailable ||
                setup.missingModels.length === 0 ||
                setup.download.status === "running"
              }
            >
              {setup.download.status === "running" ? "Downloading..." : "Download model"}
            </button>
          )}
          {canSetupOcr && (
            <button type="button" onClick={onSetupOcr}>
              Setup OCR
            </button>
          )}
          {setup.venvProvision.status === "running" && !isGroq && (
            <button type="button" disabled>
              Setting up OCR...
            </button>
          )}
          {onClose && <button type="button" onClick={onClose}>Close</button>}
        </div>
      </div>
    </ModalOverlay>
  );
}
