const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;

// ── State ─────────────────────────────────────────────────────────────────────

const queue = []; // { path, isDir, sizeBytes }

// ── DOM refs ──────────────────────────────────────────────────────────────────

const dropZone         = document.getElementById("drop-zone");
const queueSection     = document.getElementById("queue-section");
const queueList        = document.getElementById("queue-list");
const queueCount       = document.getElementById("queue-count");
const btnClearQueue    = document.getElementById("btn-clear-queue");
const btnPickFiles     = document.getElementById("btn-pick-files");
const btnPickFolder    = document.getElementById("btn-pick-folder");
const btnShred         = document.getElementById("btn-shred");
const optForce         = document.getElementById("opt-force");
const optNoTrim        = document.getElementById("opt-no-trim");
const progressOverlay  = document.getElementById("progress-overlay");
const progressBar      = document.getElementById("progress-bar");
const progressLabel    = document.getElementById("progress-label");
const resultOverlay    = document.getElementById("result-overlay");
const resultIcon       = document.getElementById("result-icon");
const resultTitle      = document.getElementById("result-title");
const resultStats      = document.getElementById("result-stats");
const resultErrors     = document.getElementById("result-errors");
const btnResultClose   = document.getElementById("btn-result-close");

// ── Queue management ──────────────────────────────────────────────────────────

function formatBytes(bytes) {
  if (bytes === 0) return "folder";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function renderQueue() {
  queueList.innerHTML = "";
  queue.forEach((item, i) => {
    const li = document.createElement("li");
    li.innerHTML = `
      <span class="queue-item-icon">${item.isDir ? "📁" : "📄"}</span>
      <span class="queue-item-path" title="${item.path}">${item.path}</span>
      <span class="queue-item-size">${formatBytes(item.sizeBytes)}</span>
      <button class="queue-item-remove" data-i="${i}" title="Remove">✕</button>
    `;
    queueList.appendChild(li);
  });

  queueList.querySelectorAll(".queue-item-remove").forEach(btn => {
    btn.addEventListener("click", () => {
      queue.splice(Number(btn.dataset.i), 1);
      renderQueue();
      syncUI();
    });
  });

  const n = queue.length;
  queueCount.textContent = `${n} item${n !== 1 ? "s" : ""}`;
  queueSection.classList.toggle("hidden", n === 0);
  syncUI();
}

function syncUI() {
  btnShred.disabled = queue.length === 0;
}

async function addPaths(paths) {
  for (const path of paths) {
    if (queue.some(q => q.path === path)) continue;
    try {
      const info = await invoke("validate_path", { path });
      queue.push({ path: info.path, isDir: info.isDir, sizeBytes: info.sizeBytes });
    } catch (err) {
      showError(path, String(err));
    }
  }
  renderQueue();
}

// ── File / folder pickers ─────────────────────────────────────────────────────

btnPickFiles.addEventListener("click", async () => {
  const selected = await open({ multiple: true, directory: false });
  if (!selected) return;
  await addPaths(Array.isArray(selected) ? selected : [selected]);
});

btnPickFolder.addEventListener("click", async () => {
  const selected = await open({ multiple: false, directory: true });
  if (!selected) return;
  await addPaths([selected]);
});

// ── Drag and drop ─────────────────────────────────────────────────────────────

dropZone.addEventListener("dragover", e => {
  e.preventDefault();
  dropZone.classList.add("drag-over");
});

dropZone.addEventListener("dragleave", () => dropZone.classList.remove("drag-over"));

dropZone.addEventListener("drop", async e => {
  e.preventDefault();
  dropZone.classList.remove("drag-over");
  const paths = Array.from(e.dataTransfer.files).map(f => f.path);
  if (paths.length) await addPaths(paths);
});

// ── Clear queue ───────────────────────────────────────────────────────────────

btnClearQueue.addEventListener("click", () => {
  queue.length = 0;
  renderQueue();
});

// ── Shred ─────────────────────────────────────────────────────────────────────

btnShred.addEventListener("click", async () => {
  if (queue.length === 0) return;

  showProgress(0, `Shredding ${queue.length} item${queue.length !== 1 ? "s" : ""}…`);

  const paths = queue.map(q => q.path);

  // Animate progress bar while work runs in the backend
  let fakePct = 0;
  const ticker = setInterval(() => {
    fakePct = Math.min(fakePct + (100 - fakePct) * 0.06, 92);
    setProgress(fakePct, `Shredding ${queue.length} item${queue.length !== 1 ? "s" : ""}…`);
  }, 200);

  try {
    const resp = await invoke("shred", {
      req: {
        paths,
        force: optForce.checked,
        no_trim: optNoTrim.checked,
      },
    });

    clearInterval(ticker);
    setProgress(100, "Done.");

    await new Promise(r => setTimeout(r, 400));
    hideProgress();

    const failed = resp.results.filter(r => !r.success);
    showResult(resp, failed);

    queue.length = 0;
    renderQueue();
  } catch (err) {
    clearInterval(ticker);
    hideProgress();
    showError("Shred failed", String(err));
  }
});

// ── Progress helpers ──────────────────────────────────────────────────────────

function showProgress(pct, label) {
  progressBar.style.width = `${pct}%`;
  progressLabel.textContent = label;
  progressOverlay.classList.remove("hidden");
}

function setProgress(pct, label) {
  progressBar.style.width = `${pct}%`;
  progressLabel.textContent = label;
}

function hideProgress() {
  progressOverlay.classList.add("hidden");
}

// ── Result helpers ────────────────────────────────────────────────────────────

function showResult(resp, failed) {
  const success = resp.results.length - failed.length;
  const hasErrors = failed.length > 0;

  resultIcon.textContent = hasErrors ? (success > 0 ? "⚠️" : "❌") : "✅";
  resultTitle.textContent = hasErrors
    ? (success > 0 ? "Completed with errors" : "Failed")
    : "Shredded successfully";

  const speedStr = resp.avg_speed_mb_s > 0
    ? `  ·  avg ${resp.avg_speed_mb_s.toFixed(0)} MB/s`
    : "";
  resultStats.textContent =
    `${success} file${success !== 1 ? "s" : ""} shredded  ·  ${resp.total_mb.toFixed(1)} MB${speedStr}`;

  resultErrors.innerHTML = "";
  if (hasErrors) {
    resultErrors.classList.remove("hidden");
    failed.forEach(r => {
      const li = document.createElement("li");
      li.textContent = `${r.path}: ${r.error || "unknown error"}`;
      resultErrors.appendChild(li);
    });
  } else {
    resultErrors.classList.add("hidden");
  }

  resultOverlay.classList.remove("hidden");
}

function showError(path, msg) {
  resultIcon.textContent = "❌";
  resultTitle.textContent = "Error";
  resultStats.textContent = path;
  resultErrors.innerHTML = `<li>${msg}</li>`;
  resultErrors.classList.remove("hidden");
  resultOverlay.classList.remove("hidden");
}

btnResultClose.addEventListener("click", () => {
  resultOverlay.classList.add("hidden");
});

// ── Init ──────────────────────────────────────────────────────────────────────

renderQueue();
