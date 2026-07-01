const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;
const { open: openExternal } = window.__TAURI__.shell;
const { listen } = window.__TAURI__.event;


// ── State ─────────────────────────────────────────────────────────────────────

let section = "shred"; // "shred" | "about"
let mode = "folder";   // "folder" | "drive" | "physical"
let folderPath = null;
let drives = [];
let selectedDrive = null;

// Physical drive state
let physDrives = [];
let selectedPhysDrive = null;
let physProgressUnlisten = null;
let sanitizePoller = null;

// ── DOM ─────────────────────────────────────────────────────────────────────────

const sectionShredTab = document.getElementById("section-shred");
const sectionAboutTab = document.getElementById("section-about");
const viewShred = document.getElementById("view-shred");
const viewAbout = document.getElementById("view-about");

const tabFolder = document.getElementById("tab-folder");
const tabDrive = document.getElementById("tab-drive");
const tabPhysical = document.getElementById("tab-physical");
const panelFolder = document.getElementById("panel-folder");
const panelDrive = document.getElementById("panel-drive");
const panelPhysical = document.getElementById("panel-physical");

// Physical panel elements
const physDriveSelect = document.getElementById("phys-drive-select");
const btnPhysRefresh = document.getElementById("btn-phys-refresh");
const physDriveTarget = document.getElementById("phys-drive-target");
const physDriveBadge = document.getElementById("phys-drive-badge");
const physDriveModel = document.getElementById("phys-drive-model");
const physDriveSize = document.getElementById("phys-drive-size");
const physDriveTypeBadge = document.getElementById("phys-drive-type-badge");
const physMethodField = document.getElementById("phys-method-field");
const physSystemWarn = document.getElementById("phys-system-warn");
const physSanitizeStatus = document.getElementById("phys-sanitize-status");
const physStatusLabel = document.getElementById("phys-status-label");
const physStatusBar = document.getElementById("phys-status-bar");
const btnPhysPoll = document.getElementById("btn-phys-poll");
const btnPhysAction = document.getElementById("btn-phys-action");
const btnPhysLabel = document.getElementById("btn-phys-label");

const physProgressOverlay = document.getElementById("phys-progress-overlay");
const physProgressTitle = document.getElementById("phys-progress-title");
const physProgressDevice = document.getElementById("phys-progress-device");
const physProgressBar = document.getElementById("phys-progress-bar");
const physProgressPct = document.getElementById("phys-progress-pct");
const physProgressDone = document.getElementById("phys-progress-done");
const physProgressSpeed = document.getElementById("phys-progress-speed");

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
  const isDrive = mode === "drive";
  const isPhysical = mode === "physical";

  tabFolder.classList.toggle("active", isFolder);
  tabDrive.classList.toggle("active", isDrive);
  tabPhysical.classList.toggle("active", isPhysical);
  tabFolder.setAttribute("aria-selected", isFolder);
  tabDrive.setAttribute("aria-selected", isDrive);
  tabPhysical.setAttribute("aria-selected", isPhysical);

  panelFolder.classList.toggle("active", isFolder);
  panelDrive.classList.toggle("active", isDrive);
  panelPhysical.classList.toggle("active", isPhysical);
  panelFolder.hidden = !isFolder;
  panelDrive.hidden = !isDrive;
  panelPhysical.hidden = !isPhysical;

  // The main shred button is only for folder/drive modes
  const shredBtn = document.getElementById("btn-shred");
  if (shredBtn) shredBtn.style.display = isPhysical ? "none" : "";
  if (btnPhysAction) btnPhysAction.style.display = isPhysical ? "" : "none";

  if (isPhysical && physDrives.length === 0) loadPhysicalDrives();

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
tabPhysical.addEventListener("click", () => setMode("physical"));

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

// ── Physical drive mode ───────────────────────────────────────────────────────

async function loadPhysicalDrives() {
  physDriveSelect.disabled = true;
  physDriveSelect.innerHTML = '<option value="">Scanning physical drives…</option>';
  try {
    physDrives = await invoke("list_physical_drives");
    physDriveSelect.innerHTML = "";

    const placeholder = document.createElement("option");
    placeholder.value = "";
    placeholder.textContent = physDrives.length
      ? "Select a physical drive…"
      : "No physical drives found";
    physDriveSelect.appendChild(placeholder);

    for (const d of physDrives) {
      const opt = document.createElement("option");
      opt.value = d.path;
      const sysTag = d.is_system ? " [SYSTEM]" : "";
      opt.textContent = `${d.model || d.path}  ${formatBytes(d.size_bytes)}  [${d.media_type}]${sysTag}`;
      if (d.is_system) opt.style.color = "var(--danger)";
      physDriveSelect.appendChild(opt);
    }

    physDriveSelect.disabled = physDrives.length === 0;
  } catch (err) {
    physDriveSelect.innerHTML = '<option value="">Failed to list drives</option>';
    showError("Physical drive scan failed", String(err));
  }
}

function getSelectedPhysMethod() {
  const checked = document.querySelector('input[name="phys-method"]:checked');
  return checked ? checked.value : "raw_sector";
}

function updatePhysicalUI() {
  const path = physDriveSelect.value;
  selectedPhysDrive = physDrives.find(d => d.path === path) ?? null;

  if (!selectedPhysDrive) {
    physDriveTarget.classList.add("hidden");
    physMethodField.hidden = true;
    physSystemWarn.classList.add("hidden");
    btnPhysAction.disabled = true;
    btnPhysLabel.textContent = "Select a drive";
    return;
  }

  // Show drive info card
  physDriveTarget.classList.remove("hidden", "empty");
  physDriveTarget.classList.add("selected");

  const shortName = selectedPhysDrive.path.replace(/.*[/\\]/, "");
  physDriveBadge.textContent = shortName.replace("PhysicalDrive", "PD") || "D";
  physDriveModel.textContent = selectedPhysDrive.model || selectedPhysDrive.path;
  physDriveSize.textContent = formatBytes(selectedPhysDrive.size_bytes);

  const type = selectedPhysDrive.media_type;
  physDriveTypeBadge.textContent = type;
  physDriveTypeBadge.className = "tag " + mediaTypeClass(type);

  if (selectedPhysDrive.is_system) {
    physSystemWarn.classList.remove("hidden");
    physMethodField.hidden = true;
    btnPhysAction.disabled = true;
    btnPhysLabel.textContent = "System drive — protected";
    return;
  }

  physSystemWarn.classList.add("hidden");
  physMethodField.hidden = false;
  syncPhysButton();
}

function mediaTypeClass(type) {
  if (type === "NVMe") return "tag-nvme";
  if (type.includes("SSD")) return "tag-ssd";
  if (type === "HDD") return "tag-hdd";
  if (type === "USB") return "tag-usb";
  return "";
}

function syncPhysButton() {
  if (!selectedPhysDrive || selectedPhysDrive.is_system) return;
  const method = getSelectedPhysMethod();
  const labels = {
    raw_sector:  "Raw Sector Wipe",
    nvme_crypto: "NVMe Crypto Erase",
    nvme_block:  "NVMe Block Erase",
    bootable:    "Export Bootable Script",
  };
  btnPhysLabel.textContent = labels[method] || "Execute";
  btnPhysAction.disabled = false;
}

document.querySelectorAll('input[name="phys-method"]').forEach(radio => {
  radio.addEventListener("change", syncPhysButton);
});

physDriveSelect.addEventListener("change", updatePhysicalUI);
btnPhysRefresh.addEventListener("click", loadPhysicalDrives);

// ── Physical action button ────────────────────────────────────────────────────

btnPhysAction.addEventListener("click", () => {
  if (!selectedPhysDrive) return;
  const method = getSelectedPhysMethod();
  if (method === "bootable") {
    exportBootableScript();
  } else {
    showPhysConfirm(method);
  }
});

function showPhysConfirm(method) {
  const methodNames = {
    raw_sector:  "Raw Sector Wipe",
    nvme_crypto: "NVMe Sanitize — Crypto Erase",
    nvme_block:  "NVMe Sanitize — Block Erase",
  };
  const methodDesc = {
    raw_sector:
      "Every sector on the physical disk will be overwritten with zeros. " +
      "This completely destroys all data at the hardware level, bypassing the filesystem. " +
      "Cannot be undone.",
    nvme_crypto:
      "The drive firmware will destroy its own encryption keys, making all data permanently " +
      "unreadable. This is the most thorough erase for NVMe SSDs. Cannot be undone.",
    nvme_block:
      "The drive firmware will erase every NAND block via the internal Sanitize command. " +
      "For NVMe drives without per-sector encryption. Cannot be undone.",
  };

  confirmTitle.textContent = `${methodNames[method]}?`;
  confirmDesc.textContent = methodDesc[method] || "Permanent data destruction.";
  confirmPath.textContent = `${selectedPhysDrive.path}  (${selectedPhysDrive.model})`;

  const keyword = physConfirmKeyword();
  confirmKeyword.textContent = keyword;
  confirmDriveExtra.classList.remove("hidden");
  confirmInput.value = "";
  btnConfirmProceed.disabled = true;

  // Store method for confirm handler
  btnConfirmProceed.dataset.physMethod = method;
  btnConfirmProceed.dataset.physMode = "1";

  confirmOverlay.classList.remove("hidden");
  confirmInput.focus();
}

function physConfirmKeyword() {
  if (!selectedPhysDrive) return "ERASE";
  const model = selectedPhysDrive.model.trim().split(/\s+/)[0].toUpperCase();
  return model.length >= 3 && model.length <= 12 ? model : "ERASE";
}

// Patch confirm input validation for physical mode
const _origConfirmInput = confirmInput.oninput;
confirmInput.addEventListener("input", () => {
  if (btnConfirmProceed.dataset.physMode === "1") {
    const keyword = physConfirmKeyword();
    btnConfirmProceed.disabled = confirmInput.value.trim().toUpperCase() !== keyword;
  }
});

// Patch confirm proceed for physical mode
const _origProceedClick = btnConfirmProceed.onclick;
btnConfirmProceed.addEventListener("click", async () => {
  if (btnConfirmProceed.dataset.physMode !== "1") return;
  delete btnConfirmProceed.dataset.physMode;
  const method = btnConfirmProceed.dataset.physMethod;
  delete btnConfirmProceed.dataset.physMethod;
  confirmOverlay.classList.add("hidden");
  await executePhysicalErase(method);
});

async function executePhysicalErase(method) {
  if (!selectedPhysDrive) return;

  if (method === "raw_sector") {
    await runRawSectorWipe();
  } else if (method === "nvme_crypto" || method === "nvme_block") {
    await runNvmeSanitize(method);
  }
}

// ── Raw sector wipe ───────────────────────────────────────────────────────────

async function runRawSectorWipe() {
  if (!selectedPhysDrive) return;

  physProgressTitle.textContent = "Raw Sector Wipe in progress…";
  physProgressDevice.textContent = `${selectedPhysDrive.path}  ·  ${selectedPhysDrive.model}`;
  physProgressBar.style.width = "0%";
  physProgressPct.textContent = "0%";
  physProgressDone.textContent = "0 GB / 0 GB";
  physProgressSpeed.textContent = "— MB/s";
  physProgressOverlay.classList.remove("hidden");

  // Subscribe to progress events
  if (physProgressUnlisten) physProgressUnlisten();
  physProgressUnlisten = await listen("physical-wipe-progress", (event) => {
    const { bytes_done, total_bytes, pct, speed_mb_s } = event.payload;
    physProgressBar.style.width = `${pct}%`;
    physProgressPct.textContent = `${pct}%`;
    physProgressDone.textContent = `${formatBytes(bytes_done)} / ${formatBytes(total_bytes)}`;
    physProgressSpeed.textContent = speed_mb_s > 0
      ? `${speed_mb_s.toFixed(0)} MB/s`
      : "— MB/s";
  });

  try {
    const result = await invoke("raw_sector_wipe", {
      req: { drive_path: selectedPhysDrive.path },
    });

    physProgressOverlay.classList.add("hidden");
    physProgressUnlisten && physProgressUnlisten();
    physProgressUnlisten = null;

    showPhysResult(
      "success",
      "Drive wiped",
      `${formatBytes(result.bytes_wiped)} overwritten with zeros · ${result.speed_mb_s.toFixed(0)} MB/s avg`
    );
  } catch (err) {
    physProgressOverlay.classList.add("hidden");
    physProgressUnlisten && physProgressUnlisten();
    physProgressUnlisten = null;
    showError("Raw sector wipe failed", String(err));
  }
}

// ── NVMe sanitize ─────────────────────────────────────────────────────────────

async function runNvmeSanitize(method) {
  if (!selectedPhysDrive) return;
  try {
    const result = await invoke("hardware_secure_erase", {
      req: { drive_path: selectedPhysDrive.path, method },
    });
    // Command accepted — show sanitize status poller
    physSanitizeStatus.classList.remove("hidden");
    physStatusLabel.textContent = "Sanitize command accepted — firmware is erasing…";
    physStatusBar.style.width = "0%";
    startSanitizePoller();
    showPhysResult(
      "info",
      "Sanitize command sent",
      result.message
    );
  } catch (err) {
    showError("NVMe Sanitize failed", String(err));
  }
}

function startSanitizePoller() {
  if (sanitizePoller) clearInterval(sanitizePoller);
  sanitizePoller = setInterval(async () => {
    if (!selectedPhysDrive) { clearInterval(sanitizePoller); return; }
    try {
      const pct = await invoke("nvme_sanitize_status", {
        drivePath: selectedPhysDrive.path,
      });
      if (pct === null) {
        // No status — either not started or already done
        physStatusLabel.textContent = "Sanitize complete (or no status available).";
        physStatusBar.style.width = "100%";
        clearInterval(sanitizePoller);
      } else {
        physStatusLabel.textContent = `Sanitizing… ${pct}%`;
        physStatusBar.style.width = `${pct}%`;
        if (pct >= 100) {
          physStatusLabel.textContent = "Sanitize complete.";
          clearInterval(sanitizePoller);
        }
      }
    } catch (_) {}
  }, 3000);
}

btnPhysPoll.addEventListener("click", async () => {
  if (!selectedPhysDrive) return;
  try {
    const pct = await invoke("nvme_sanitize_status", {
      drivePath: selectedPhysDrive.path,
    });
    if (pct === null) {
      physStatusLabel.textContent = "No sanitize in progress (complete or never started).";
      physStatusBar.style.width = "100%";
    } else {
      physStatusLabel.textContent = `Sanitizing… ${pct}%`;
      physStatusBar.style.width = `${pct}%`;
    }
  } catch (err) {
    physStatusLabel.textContent = `Error polling status: ${err}`;
  }
});

// ── Bootable script export ────────────────────────────────────────────────────

async function exportBootableScript() {
  if (!selectedPhysDrive) return;

  // Ask where to save
  // Ask user to pick a folder for the package output
  const outputDir = await open({
    title: "Choose folder for bootable wipe package",
    directory: true,
    multiple: false,
  });
  if (!outputDir) return;

  try {
    const msg = await invoke("export_bootable_script", {
      req: {
        output_dir: outputDir,
        device_paths: [selectedPhysDrive.path],
        passes: 1,
      },
    });
    showPhysResult("success", "Bootable script exported", msg +
      "\n\nBoot from a live Linux USB (ShredOS, Ubuntu Live) and run:\n  sudo bash nitroshred-wipe.sh"
    );
  } catch (err) {
    showError("Export failed", String(err));
  }
}

// ── Physical result helper ────────────────────────────────────────────────────

function showPhysResult(type, title, message) {
  const iconMap = {
    success: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 6 9 17l-5-5"/></svg>`,
    info:    `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 8h.01"/></svg>`,
    error:   `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6 6 18M6 6l12 12"/></svg>`,
  };
  resultIcon.className = `modal-icon ${type === "info" ? "warn" : type}`;
  resultIcon.innerHTML = iconMap[type] || iconMap.info;
  resultTitle.textContent = title;
  resultStats.textContent = message;
  resultErrors.classList.add("hidden");
  resultOverlay.classList.remove("hidden");
}

// ── External links ────────────────────────────────────────────────────────────

document.querySelectorAll(".support-links a").forEach(link => {
  link.addEventListener("click", async e => {
    e.preventDefault();
    await openExternal(link.href);
  });
});

// ── Init ──────────────────────────────────────────────────────────────────────

setSection("shred");
setMode("folder"); // also hides btnPhysAction via setMode logic
loadDrives();
syncShredButton();
