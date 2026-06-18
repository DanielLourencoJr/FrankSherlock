import { useCallback, useEffect, useMemo, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listManualFiles, listRoots, saveManualDescription } from "../../api";
import type { RootInfo, UnclassifiedFileInfo } from "../../types";
import { fileName } from "../../utils/format";
import "./shared-tool-view.css";
import "./ManualDescriptionView.css";

const DEFAULT_PROMPT = `Describe this image in detail. Include what is shown (objects, people, setting), colors, composition, any visible text, mood/atmosphere. Be specific and thorough.

If this is from an anime, manga, or illustrated work, also identify:
- The series/franchise name (canonical name)
- Any characters present (full canonical names)
- A brief scene summary`;

function detectMediaType(absPath: string): string {
  const ext = absPath.split(".").pop()?.toLowerCase() ?? "";
  if (["jpg", "jpeg", "png", "webp", "bmp", "tiff", "tif", "heic", "heif", "avif"].includes(ext)) return "photo";
  if (["gif"].includes(ext)) return "photo";
  if (["mp4", "webm", "mov", "avi", "mkv", "wmv", "flv"].includes(ext)) return "video";
  if (["pdf"].includes(ext)) return "document";
  return "other";
}

type Props = {
  onBack: () => void;
  onNotice: (msg: string) => void;
  onError: (msg: string) => void;
};

export default function ManualDescriptionView({ onBack, onNotice, onError }: Props) {
  const [roots, setRoots] = useState<RootInfo[]>([]);
  const [selectedRootId, setSelectedRootId] = useState<number | null>(null);
  const [files, setFiles] = useState<UnclassifiedFileInfo[]>([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [description, setDescription] = useState("");
  const [prompt, setPrompt] = useState(DEFAULT_PROMPT);
  const [saving, setSaving] = useState(false);
  const [loading, setLoading] = useState(true);

  const currentFile = files[currentIndex] ?? null;

  const fileBasename = currentFile ? fileName(currentFile.relPath) : "";
  const fileDirname = currentFile ? currentFile.relPath.substring(0, currentFile.relPath.lastIndexOf("/")) : "";
  const totalCount = files.length;
  const remainingCount = files.length - currentIndex;

  useEffect(() => {
    listRoots()
      .then((r) => {
        setRoots(r);
        if (r.length === 1) setSelectedRootId(r[0].id);
      })
      .catch((e) => onError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    if (selectedRootId == null) return;
    setLoading(true);
    setCurrentIndex(0);
    setDescription("");
    listManualFiles(selectedRootId)
      .then((f) => setFiles(f))
      .catch((e) => onError(String(e)))
      .finally(() => setLoading(false));
  }, [selectedRootId]);

  function goNext() {
    if (currentIndex < files.length - 1) {
      setCurrentIndex((i) => i + 1);
      setDescription("");
    }
  }

  const handleSave = useCallback(async () => {
    if (!currentFile || !description.trim()) return;
    setSaving(true);
    try {
      const mediaType = detectMediaType(currentFile.absPath);
      await saveManualDescription(currentFile.id, description.trim(), mediaType);
      onNotice("Description saved");
      goNext();
    } catch (e) {
      onError(String(e));
    } finally {
      setSaving(false);
    }
  }, [currentFile, description]);

  const handleSkip = useCallback(() => {
    goNext();
  }, [currentIndex, files.length]);

  async function handleCopyPrompt() {
    try {
      await navigator.clipboard.writeText(prompt);
      onNotice("Prompt copied to clipboard");
    } catch {
      onError("Failed to copy prompt");
    }
  }

  if (loading && files.length === 0) {
    return (
      <div className="tool-view">
        <div className="tool-toolbar">
          <button type="button" onClick={onBack}>Back to Gallery</button>
          <div className="tool-toolbar-stats">Manual Description</div>
        </div>
        <div className="tool-body">
          <div className="tool-loading">Loading...</div>
        </div>
      </div>
    );
  }

  if (!loading && roots.length === 0) {
    return (
      <div className="tool-view">
        <div className="tool-toolbar">
          <button type="button" onClick={onBack}>Back to Gallery</button>
          <div className="tool-toolbar-stats">Manual Description</div>
        </div>
        <div className="tool-body">
          <div className="tool-empty">
            <p>No folders added yet. Add a folder and run a quick scan first.</p>
          </div>
        </div>
      </div>
    );
  }

  if (selectedRootId != null && !loading && totalCount === 0) {
    return (
      <div className="tool-view">
        <div className="tool-toolbar">
          <button type="button" onClick={onBack}>Back to Gallery</button>
          <div className="tool-toolbar-stats">
            <strong>Manual Description</strong> &mdash; {roots.find((r) => r.id === selectedRootId)?.rootName ?? "Unknown"}
          </div>
        </div>
        <div className="tool-body">
          <div className="tool-empty">
            <p>All images in this folder have been described!</p>
          </div>
        </div>
      </div>
    );
  }

  if (currentIndex >= files.length && totalCount > 0) {
    return (
      <div className="tool-view">
        <div className="tool-toolbar">
          <button type="button" onClick={onBack}>Back to Gallery</button>
          <div className="tool-toolbar-stats">
            <strong>Manual Description</strong> &mdash; {roots.find((r) => r.id === selectedRootId)?.rootName ?? "Unknown"}
          </div>
        </div>
        <div className="tool-body">
          <div className="tool-empty">
            <p>All done! All {totalCount} images have been described.</p>
            <button type="button" className="md-back-btn" onClick={onBack}>Back to Gallery</button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="tool-view">
      <div className="tool-toolbar">
        <button type="button" onClick={onBack}>Back to Gallery</button>
        <div className="tool-toolbar-stats">
          <strong>Manual Description</strong>
          {roots.length > 1 && (
            <select
              className="md-root-select"
              value={selectedRootId ?? ""}
              onChange={(e) => setSelectedRootId(Number(e.target.value) || null)}
            >
              <option value="" disabled>Select folder...</option>
              {roots.map((r) => (
                <option key={r.id} value={r.id}>{r.rootName}</option>
              ))}
            </select>
          )}
          {totalCount > 0 && (
            <span> &mdash; <strong>{currentIndex + 1}</strong> of <strong>{totalCount}</strong> images ({(totalCount - currentIndex - 1)} remaining)</span>
          )}
        </div>
      </div>
      <div className="tool-body md-body">
        {currentFile && (
          <>
            <div className="md-preview-section">
              <div className="md-image-wrap">
                <img
                  className="md-image"
                  src={convertFileSrc(currentFile.absPath)}
                  alt={fileBasename}
                />
              </div>
              <div className="md-file-info">
                <strong>{fileBasename}</strong>
                {fileDirname && <span className="md-dirname">{fileDirname}</span>}
              </div>
            </div>

            <div className="md-prompt-section">
              <label className="md-label">Prompt (copy to AI chat)</label>
              <textarea
                className="md-textarea md-prompt-textarea"
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                rows={8}
              />
              <button type="button" className="md-copy-btn" onClick={handleCopyPrompt}>
                Copy Prompt
              </button>
            </div>

            <div className="md-description-section">
              <label className="md-label">Description (paste from AI chat)</label>
              <textarea
                className="md-textarea md-description-textarea"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="Paste the AI's description here..."
                rows={6}
              />
            </div>

            <div className="md-actions">
              <button
                type="button"
                className="md-save-btn"
                onClick={handleSave}
                disabled={saving || !description.trim()}
              >
                {saving ? "Saving..." : "Save & Next"}
              </button>
              <button
                type="button"
                className="md-skip-btn"
                onClick={handleSkip}
                disabled={saving}
              >
                Skip
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
