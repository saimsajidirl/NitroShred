const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;
const { open: openExternal } = window.__TAURI__.shell;

// ── State ─────────────────────────────────────────────────────────────────────

let section = "shred"; // "shred" | "about"
let mode = "folder"; // "folder" | "drive"
let folderPath = null;
let drives = [];
let selectedDrive = null;

// ── DOM ─────────────────────────────────────────────────────────────────────────

const sectionShredTab = document.getElementById("section-shred");
const sectionAboutTab = document.getElementById("section-about");
const viewShred = document.getElementById("view-shred");
const viewAbout = document.getElementById("view-about");

const tabFolder = document.getElementById("tab-folder");
const tabDrive = document.getElementById("tab-drive");
const panelFolder = document.getElementById("panel-folder");
const panelDrive = document.getElementById("panel-drive");

const folderTarget = document.getElementById("folder-target");
const folderPathEl = document.getElementById("folder-path");
const btnSelectFolder = document.getElementById("btn-select-folder");

const driveSelect = document.getElementById("drive-select");
const driveTarget = document.getElementById("drive-target");
const driveLetter = document.getElementById("drive-letter");
const driveLabel = document.getElementById("drive-label");
const driveSize = document.getElementById("drive-size");
const driveWarning = document.getElementById("drive-warning");

const btnShred = document.getElementById("btn-shred");
const btnShredLabel = document.getElementById("btn-shred-label");

const confirmOverlay = document.getElementById("confirm-overlay");
const confirmTitle = document.getElementById("confirm-title");
const confirmDesc = document.getElementById("confirm-desc");
const confirmPath = document.getElementById("confirm-path");
const confirmDriveExtra = document.getElementById("confirm-drive-extra");
const confirmKeyword = document.getElementById("confirm-keyword");
const confirmInput = document.getElementById("confirm-input");
const btnConfirmCancel = document.getElementById("btn-confirm-cancel");
const btnConfirmProceed = document.getElementById("btn-confirm-proceed");

const progressOverlay = document.getElementById("progress-overlay");
const progressBar = document.getElementById("progress-bar");
const progressLabel = document.getElementById("progress-label");

const resultOverlay = document.getElementById("result-overlay");
const resultIcon = document.getElementById("result-icon");
const resultTitle = document.getElementById("result-title");
const resultStats = document.getElementById("result-stats");
const resultErrors = document.getElementById("result-errors");
const btnResultClose = document.getElementById("btn-result-close");

// ── Helpers ───────────────────────────────────────────────────────────────────

function formatBytes(bytes) {
  if (!bytes) return "Unknown size";
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(0)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

function currentTarget() {
  if (mode === "folder") return folderPath;
  return selectedDrive?.path ?? null;
}

function syncShredButton() {
  const target = currentTarget();
  const driveBlocked = mode === "drive" && selectedDrive?.is_system;
  btnShred.disabled = !target || driveBlocked;
  btnShredLabel.textContent = mode === "folder" ? "Shred folder" : "Wipe drive";
}

function setSection(next) {
  section = next;
  const isShred = section === "shred";

  sectionShredTab.classList.toggle("active", isShred);
  sectionAboutTab.classList.toggle("active", !isShred);
  sectionShredTab.setAttribute("aria-selected", isShred);
  sectionAboutTab.setAttribute("aria-selected", !isShred);

  viewShred.classList.toggle("active", isShred);
  viewAbout.classList.toggle("active", !isShred);
  viewShred.hidden = !isShred;
  viewAbout.hidden = isShred;
}

function setMode(next) {
  mode = next;
  const isFolder = mode === "folder";

  tabFolder.classList.toggle("active", isFolder);
  tabDrive.classList.toggle("active", !isFolder);
  tabFolder.setAttribute("aria-selected", isFolder);
  tabDrive.setAttribute("aria-selected", !isFolder);

  panelFolder.classList.toggle("active", isFolder);
  panelDrive.classList.toggle("active", !isFolder);
  panelFolder.hidden = !isFolder;
  panelDrive.hidden = isFolder;

  syncShredButton();
}

// ── Folder mode ───────────────────────────────────────────────────────────────

btnSelectFolder.addEventListener("click", async () => {
  const selected = await open({ multiple: false, directory: true });
  if (!selected) return;

  try {
    const info = await invoke("validate_path", { path: selected });
    folderPath = info.path;
    folderPathEl.textContent = info.path;
    folderTarget.classList.remove("empty");
    folderTarget.classList.add("selected");
    syncShredButton();
  } catch (err) {
    showError("Invalid folder", String(err));
  }
});

// ── Drive mode ────────────────────────────────────────────────────────────────

async function loadDrives() {
  try {
    drives = await invoke("list_drives");
    driveSelect.innerHTML = "";

    const placeholder = document.createElement("option");
    placeholder.value = "";
    placeholder.textContent = "Select a drive…";
    driveSelect.appendChild(placeholder);

    const usable = drives.filter(d => !d.is_system);
    for (const d of usable) {
      const opt = document.createElement("option");
      opt.value = d.path;
      opt.textContent = `${d.letter}  ${d.label}  (${formatBytes(d.total_bytes)})`;
      driveSelect.appendChild(opt);
    }

    driveSelect.disabled = usable.length === 0;
    if (usable.length === 0) {
      driveWarning.classList.remove("hidden");
      driveWarning.querySelector("span").textContent =
        "No non-system drives available. System drives are protected.";
    }
  } catch (err) {
    driveSelect.innerHTML = '<option value="">Failed to load drives</option>';
    showError("Drive scan failed", String(err));
  }
}

function updateDriveUI() {
  const path = driveSelect.value;
  selectedDrive = drives.find(d => d.path === path) ?? null;

  if (!selectedDrive) {
    driveTarget.classList.add("hidden");
    driveWarning.classList.add("hidden");
    syncShredButton();
    return;
  }

  driveTarget.classList.remove("hidden", "empty");
  driveTarget.classList.add("selected");
  driveLetter.textContent = selectedDrive.letter;
  driveLabel.textContent = selectedDrive.label;
  driveSize.textContent = formatBytes(selectedDrive.total_bytes);

  driveWarning.classList.toggle("hidden", !selectedDrive.is_system);
  syncShredButton();
}

driveSelect.addEventListener("change", updateDriveUI);

// ── Section tabs ──────────────────────────────────────────────────────────────

sectionShredTab.addEventListener("click", () => setSection("shred"));
sectionAboutTab.addEventListener("click", () => setSection("about"));

// ── Mode tabs ─────────────────────────────────────────────────────────────────

tabFolder.addEventListener("click", () => setMode("folder"));
tabDrive.addEventListener("click", () => setMode("drive"));

function driveConfirmKeyword(drive) {
  if (!drive) return "";
  if (drive.letter.includes(":")) {
    return drive.letter.replace(":", "").toUpperCase();
  }
  const parts = drive.path.split(/[/\\]/).filter(Boolean);
  return (parts.at(-1) ?? drive.label).toUpperCase();
}

// ── Shred flow ────────────────────────────────────────────────────────────────

btnShred.addEventListener("click", () => {
  const target = currentTarget();
  if (!target) return;

  const isDrive = mode === "drive";

  confirmTitle.textContent = isDrive
    ? "Wipe entire drive?"
    : "Shred folder contents?";

  confirmDesc.textContent = isDrive
    ? "This runs a full secure wipe in three phases: (1) shred every file with zeros, (2) overwrite all remaining free space, (3) TRIM the drive at the hardware level. This cannot be undone."
    : "All files inside this folder will be overwritten with zeros and permanently deleted. This cannot be undone.";

  confirmPath.textContent = target;

  if (isDrive) {
    const keyword = driveConfirmKeyword(selectedDrive);
    confirmKeyword.textContent = keyword;
    confirmDriveExtra.classList.remove("hidden");
    confirmInput.value = "";
    btnConfirmProceed.disabled = true;
  } else {
    confirmDriveExtra.classList.add("hidden");
    btnConfirmProceed.disabled = false;
  }

  confirmOverlay.classList.remove("hidden");
  if (isDrive) confirmInput.focus();
});

confirmInput.addEventListener("input", () => {
  const keyword = driveConfirmKeyword(selectedDrive);
  btnConfirmProceed.disabled = confirmInput.value.trim().toUpperCase() !== keyword;
});

btnConfirmCancel.addEventListener("click", () => {
  confirmOverlay.classList.add("hidden");
});

btnConfirmProceed.addEventListener("click", async () => {
  confirmOverlay.classList.add("hidden");
  await runShred();
});

async function runShred() {
  const target = currentTarget();
  if (!target) return;

  showProgress(0, mode === "drive"
    ? "Phase 1/3 — shredding files…"
    : "Shredding folder…");

  let fakePct = 0;
  const ticker = setInterval(() => {
    fakePct = Math.min(fakePct + (100 - fakePct) * 0.05, 92);
    setProgress(fakePct, mode === "drive"
      ? "Secure wipe in progress…"
      : "Shredding folder…");
  }, 250);

  try {
    const resp = await invoke("shred", {
      req: {
        paths: [target],
        full_drive: mode === "drive",
      },
    });

    clearInterval(ticker);
    setProgress(100, "Complete");
    await new Promise(r => setTimeout(r, 350));
    hideProgress();

    const failed = resp.results.filter(r => !r.success);
    showResult(resp);

    if (mode === "folder") {
      folderPath = null;
      folderPathEl.textContent = "No folder selected";
      folderTarget.classList.add("empty");
      folderTarget.classList.remove("selected");
    } else {
      driveSelect.value = "";
      updateDriveUI();
      await loadDrives();
    }
    syncShredButton();
  } catch (err) {
    clearInterval(ticker);
    hideProgress();
    showError("Operation failed", String(err));
  }
}

// ── Progress ──────────────────────────────────────────────────────────────────

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

// ── Results ─────────────────────────────────────────────────────────────────

function showResult(resp) {
  const fileResults = resp.results.filter(r =>
    !r.path.includes("[free space]") && !r.path.includes("[volume TRIM]")
  );
  const freeSpace = resp.results.find(r => r.path.includes("[free space]"));
  const volumeTrim = resp.results.find(r => r.path.includes("[volume TRIM]"));

  const success = fileResults.filter(r => r.success).length;
  const failed = fileResults.filter(r => !r.success);
  const hasErrors = failed.length > 0
    || (freeSpace && !freeSpace.success)
    || (volumeTrim && !volumeTrim.success);

  resultIcon.className = "modal-icon " + (hasErrors ? (success > 0 ? "warn" : "error") : "success");
  resultIcon.innerHTML = hasErrors
    ? `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 9v4M12 17h.01"/><path d="M10.3 4.2 2.6 18a2 2 0 0 0 1.7 3h15.4a2 2 0 0 0 1.7-3L13.7 4.2a2 2 0 0 0-3.4 0z"/></svg>`
    : `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 6 9 17l-5-5"/></svg>`;

  resultTitle.textContent = hasErrors
    ? (success > 0 ? "Completed with errors" : "Failed")
    : (mode === "drive" ? "Drive securely wiped" : "Folder shredded");

  const speedStr = resp.avg_speed_mb_s > 0
    ? ` · ${resp.avg_speed_mb_s.toFixed(0)} MB/s avg`
    : "";
  let stats = `${success} file${success !== 1 ? "s" : ""} erased · ${resp.total_mb.toFixed(1)} MB${speedStr}`;
  if (freeSpace?.success && freeSpace.mb > 0) {
    stats += ` · ${freeSpace.mb.toFixed(1)} MB free space wiped`;
  }
  if (volumeTrim?.success) {
    stats += " · volume TRIM complete";
  }
  resultStats.textContent = stats;

  resultErrors.innerHTML = "";
  const allFailed = [
    ...failed,
    ...(freeSpace && !freeSpace.success ? [freeSpace] : []),
    ...(volumeTrim && !volumeTrim.success ? [volumeTrim] : []),
  ];
  if (allFailed.length > 0) {
    resultErrors.classList.remove("hidden");
    allFailed.forEach(r => {
      const li = document.createElement("li");
      li.textContent = `${r.path}: ${r.error || "unknown error"}`;
      resultErrors.appendChild(li);
    });
  } else {
    resultErrors.classList.add("hidden");
  }

  resultOverlay.classList.remove("hidden");
}

function showError(title, msg) {
  resultIcon.className = "modal-icon error";
  resultIcon.innerHTML = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6 6 18M6 6l12 12"/></svg>`;
  resultTitle.textContent = title;
  resultStats.textContent = msg;
  resultErrors.classList.add("hidden");
  resultOverlay.classList.remove("hidden");
}

btnResultClose.addEventListener("click", () => {
  resultOverlay.classList.add("hidden");
});

// ── External links ────────────────────────────────────────────────────────────

document.querySelectorAll(".support-links a").forEach(link => {
  link.addEventListener("click", async e => {
    e.preventDefault();
    await openExternal(link.href);
  });
});

// ── Init ──────────────────────────────────────────────────────────────────────

setSection("shred");
setMode("folder");
loadDrives();
syncShredButton();
