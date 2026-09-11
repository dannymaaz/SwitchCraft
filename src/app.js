/* ============================================
   SwitchCraft — Frontend Application Logic
   ============================================ */

const { invoke } = window.__TAURI__.core;
const { check } = window.__TAURI__.updater || {};
const { relaunch } = window.__TAURI__.process || {};
const { enable, disable, isEnabled } = window.__TAURI__.autostart || {};
const { open } = window.__TAURI__.shell || {};

// ── State ──────────────────────────────────────
let accounts = [];
let selectedPlatform = 'codex';
let deleteTargetId = null;
let settings = {
  autoUpdate: true,
  notifications: true,
};

// ── DOM References ─────────────────────────────
const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => document.querySelectorAll(sel);

// Titlebar
const btnMinimize = $('#btn-minimize');
const btnClose = $('#btn-close');

// Navigation
const navBtns = $$('.nav-btn');
const pages = $$('.page');

// Accounts
const platformSections = $('#platform-sections');
const emptyState = $('#empty-state');
const btnAddAccount = $('#btn-add-account');
const btnAddFirst = $('#btn-add-first');

// Modal — Add Account
const modalOverlay = $('#modal-overlay');
const btnModalClose = $('#btn-modal-close');
const inputName = $('#input-name');
const inputCreds = $('#input-creds');
const inputPlan = $('#input-plan');
const inputWeeklyLimit = $('#input-weekly-limit');
const inputHourlyLimit = $('#input-hourly-limit');
const platformPicks = $$('.platform-pick');
const btnImport = $('#btn-import');
const btnSave = $('#btn-save');

// Modal — Confirm Delete
const confirmOverlay = $('#confirm-overlay');
const btnConfirmCancel = $('#btn-confirm-cancel');
const btnConfirmDelete = $('#btn-confirm-delete');

// Settings
const toggleAutostart = $('#toggle-autostart');
const toggleAutoupdate = $('#toggle-autoupdate');
const toggleNotifications = $('#toggle-notifications');
const btnManualUpdate = $('#btn-manual-update');

// About
const versionNumber = $('#version-number');
const btnGithub = $('#btn-github');

// Update
const updateBanner = $('#update-banner');
const btnUpdate = $('#btn-update');

// Toast
const toastEl = $('#toast');
const toastMessage = $('#toast-message');

// ── Initialize ─────────────────────────────────
document.addEventListener('DOMContentLoaded', async () => {
  await loadVersion();
  await loadAccounts();
  await loadSettings();
  setupEventListeners();

  // Check for updates on startup (if enabled)
  if (settings.autoUpdate) {
    setTimeout(() => checkForUpdates(true), 2000);
  }
});

// ── Version ────────────────────────────────────
async function loadVersion() {
  try {
    const version = await invoke('get_app_version');
    versionNumber.textContent = `v${version}`;
  } catch {
    versionNumber.textContent = 'v1.0.0';
  }
}

// ── Accounts ───────────────────────────────────
async function loadAccounts() {
  try {
    accounts = await invoke('get_accounts');
  } catch {
    accounts = [];
  }
  renderAccounts();
}

function renderAccounts() {
  const platforms = ['codex', 'gemini', 'claude'];
  let totalCount = 0;

  platforms.forEach((platform) => {
    const list = $(`[data-list="${platform}"]`);
    const count = $(`[data-count="${platform}"]`);
    const section = $(`.platform-section[data-platform="${platform}"]`);
    const filtered = accounts.filter((a) => a.platform === platform);

    count.textContent = `${filtered.length} profile${filtered.length !== 1 ? 's' : ''}`;
    totalCount += filtered.length;

    if (filtered.length === 0) {
      list.innerHTML = `
        <div class="account-card" style="justify-content: center; opacity: 0.4; cursor: default;">
          <span style="font-size: 11px; color: var(--text-muted);">No profiles added</span>
        </div>`;
      return;
    }

    list.innerHTML = filtered
      .map((account) => {
        const initial = account.name.charAt(0).toUpperCase();
        const isActive = account.is_active;
        const usage = account.usage;

        let usageHTML = '';
        if (usage) {
          const weeklyPct = usage.weekly_limit
            ? Math.min(100, ((usage.weekly_used || 0) / usage.weekly_limit) * 100)
            : null;
          const hourlyPct = usage.hourly_limit
            ? Math.min(100, ((usage.hourly_used || 0) / usage.hourly_limit) * 100)
            : null;

          if (weeklyPct !== null) {
            const level = weeklyPct > 85 ? 'critical' : weeklyPct > 60 ? 'warn' : 'ok';
            usageHTML += `
              <div class="usage-bar-container">
                <div class="usage-bar"><div class="usage-bar-fill usage-bar-fill--${level}" style="width: ${weeklyPct}%"></div></div>
                <span class="usage-text">${usage.weekly_used || 0}/${usage.weekly_limit} wk</span>
              </div>`;
          }
          if (hourlyPct !== null) {
            const level = hourlyPct > 85 ? 'critical' : hourlyPct > 60 ? 'warn' : 'ok';
            usageHTML += `
              <div class="usage-bar-container">
                <div class="usage-bar"><div class="usage-bar-fill usage-bar-fill--${level}" style="width: ${hourlyPct}%"></div></div>
                <span class="usage-text">${usage.hourly_used || 0}/${usage.hourly_limit} hr</span>
              </div>`;
          }
        }

        return `
          <div class="account-card ${isActive ? 'active' : ''}" data-id="${account.id}" title="Click to switch to ${account.name}">
            <div class="account-avatar account-avatar--${account.platform}">${initial}</div>
            <div class="account-info">
              <div class="account-name">${escapeHtml(account.name)}</div>
              <div class="account-meta">
                <span class="account-status ${isActive ? 'account-status--active' : 'account-status--inactive'}">
                  ${isActive ? '● Active' : '○ Inactive'}
                </span>
                ${usage?.plan_name ? `<span class="usage-text">· ${escapeHtml(usage.plan_name)}</span>` : ''}
              </div>
              ${usageHTML}
            </div>
            <div class="account-actions">
              <button class="action-btn action-btn--delete" data-delete="${account.id}" title="Remove profile" aria-label="Remove ${account.name}">
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
              </button>
            </div>
          </div>`;
      })
      .join('');
  });

  // Show/hide empty state
  if (totalCount === 0) {
    emptyState.classList.remove('hidden');
    platformSections.style.display = 'none';
  } else {
    emptyState.classList.add('hidden');
    platformSections.style.display = 'block';
  }

  // Attach card click handlers
  document.querySelectorAll('.account-card[data-id]').forEach((card) => {
    card.addEventListener('click', (e) => {
      // Don't switch if clicking delete button
      if (e.target.closest('.action-btn')) return;
      switchAccount(card.dataset.id);
    });
  });

  // Attach delete handlers
  document.querySelectorAll('[data-delete]').forEach((btn) => {
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      deleteTargetId = btn.dataset.delete;
      const account = accounts.find((a) => a.id === deleteTargetId);
      if (account) {
        $('#confirm-message').textContent = `Remove "${account.name}" from ${account.platform}? Your actual credentials on disk won't be affected.`;
        confirmOverlay.classList.remove('hidden');
      }
    });
  });
}

async function switchAccount(id) {
  const account = accounts.find((a) => a.id === id);
  if (!account || account.is_active) return;

  try {
    await invoke('switch_account', { id });
    toast(`Switched to ${account.name}`);
    await loadAccounts();
  } catch (err) {
    toast(`Switch failed: ${err}`, true);
  }
}

async function addAccount() {
  const name = inputName.value.trim();
  const credsRaw = inputCreds.value.trim();

  if (!name) {
    toast('Enter a profile name', true);
    return;
  }

  if (!credsRaw) {
    toast('Paste your credentials or import current session', true);
    return;
  }

  let credentials;
  try {
    credentials = JSON.parse(credsRaw);
  } catch {
    toast('Invalid JSON — check your credentials format', true);
    return;
  }

  try {
    const account = await invoke('add_account', {
      name,
      platform: selectedPlatform,
      credentials,
    });

    // If usage info was provided, save it
    const planName = inputPlan.value.trim();
    const weeklyLimit = parseInt(inputWeeklyLimit.value) || null;
    const hourlyLimit = parseInt(inputHourlyLimit.value) || null;

    if (planName || weeklyLimit || hourlyLimit) {
      await invoke('update_usage', {
        id: account.id,
        usage: {
          plan_name: planName || selectedPlatform,
          weekly_limit: weeklyLimit,
          weekly_used: 0,
          hourly_limit: hourlyLimit,
          hourly_used: 0,
          reset_at: null,
        },
      });
    }

    closeModal();
    toast(`${name} added to ${selectedPlatform}`);
    await loadAccounts();
  } catch (err) {
    toast(`Failed to add account: ${err}`, true);
  }
}

async function importCurrentSession() {
  const name = inputName.value.trim();
  if (!name) {
    toast('Enter a profile name first', true);
    return;
  }

  try {
    await invoke('import_current_account', {
      name,
      platform: selectedPlatform,
    });

    closeModal();
    toast(`Imported current ${selectedPlatform} session as "${name}"`);
    await loadAccounts();
  } catch (err) {
    toast(`Import failed: ${err}`, true);
  }
}

async function deleteAccount() {
  if (!deleteTargetId) return;

  try {
    await invoke('delete_account', { id: deleteTargetId });
    confirmOverlay.classList.add('hidden');
    toast('Profile removed');
    deleteTargetId = null;
    await loadAccounts();
  } catch (err) {
    toast(`Failed to remove: ${err}`, true);
  }
}

// ── Settings ───────────────────────────────────
async function loadSettings() {
  try {
    const autoEnabled = await isEnabled();
    toggleAutostart.checked = autoEnabled;
  } catch {
    toggleAutostart.checked = false;
  }

  // Load from localStorage for simple settings
  const saved = localStorage.getItem('switchcraft_settings');
  if (saved) {
    settings = { ...settings, ...JSON.parse(saved) };
  }
  toggleAutoupdate.checked = settings.autoUpdate;
  toggleNotifications.checked = settings.notifications;
}

function saveSettings() {
  settings.autoUpdate = toggleAutoupdate.checked;
  settings.notifications = toggleNotifications.checked;
  localStorage.setItem('switchcraft_settings', JSON.stringify(settings));
}

// ── Updates ────────────────────────────────────
async function checkForUpdates(silent = false) {
  try {
    if (!check) {
      if (!silent) toast('Updater not available in this build');
      return;
    }
    const update = await check();
    if (update) {
      updateBanner.classList.remove('hidden');
      if (!silent) toast('New version available!');
    } else {
      if (!silent) toast('You\'re on the latest version');
    }
  } catch (err) {
    if (!silent) toast(`Update check failed: ${err}`, true);
  }
}

window.__checkUpdate = () => checkForUpdates(false);

async function installUpdate() {
  try {
    const update = await check();
    if (update) {
      toast('Downloading update...');
      await update.downloadAndInstall();
      toast('Update installed! Restarting...');
      setTimeout(async () => {
        if (relaunch) await relaunch();
      }, 1500);
    }
  } catch (err) {
    toast(`Update failed: ${err}`, true);
  }
}

// ── Navigation ─────────────────────────────────
function navigateTo(page) {
  navBtns.forEach((btn) => btn.classList.toggle('active', btn.dataset.page === page));
  pages.forEach((p) => {
    const isTarget = p.id === `page-${page}`;
    p.classList.toggle('active', isTarget);
  });
}

// ── Modal ──────────────────────────────────────
function openModal() {
  inputName.value = '';
  inputCreds.value = '';
  inputPlan.value = '';
  inputWeeklyLimit.value = '';
  inputHourlyLimit.value = '';
  selectedPlatform = 'codex';
  platformPicks.forEach((p) => p.classList.toggle('active', p.dataset.pick === 'codex'));
  modalOverlay.classList.remove('hidden');
  inputName.focus();
}

function closeModal() {
  modalOverlay.classList.add('hidden');
}

// ── Toast ──────────────────────────────────────
let toastTimeout;
function toast(message, isError = false) {
  clearTimeout(toastTimeout);
  toastMessage.textContent = message;
  toastEl.style.borderColor = isError ? 'var(--danger)' : 'var(--gold-dark)';
  toastEl.classList.remove('hidden');
  requestAnimationFrame(() => toastEl.classList.add('visible'));

  toastTimeout = setTimeout(() => {
    toastEl.classList.remove('visible');
    setTimeout(() => toastEl.classList.add('hidden'), 300);
  }, 3000);
}

// ── Helpers ────────────────────────────────────
function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

// ── Event Listeners ────────────────────────────
function setupEventListeners() {
  // Titlebar
  btnMinimize.addEventListener('click', async () => {
    const { getCurrentWindow } = window.__TAURI__.window;
    await getCurrentWindow().minimize();
  });
  btnClose.addEventListener('click', async () => {
    await invoke('hide_window');
  });

  // Navigation
  navBtns.forEach((btn) => {
    btn.addEventListener('click', () => navigateTo(btn.dataset.page));
  });

  // Add account
  btnAddAccount.addEventListener('click', openModal);
  btnAddFirst.addEventListener('click', openModal);
  btnModalClose.addEventListener('click', closeModal);
  modalOverlay.addEventListener('click', (e) => {
    if (e.target === modalOverlay) closeModal();
  });

  // Platform picker
  platformPicks.forEach((pick) => {
    pick.addEventListener('click', () => {
      selectedPlatform = pick.dataset.pick;
      platformPicks.forEach((p) => p.classList.toggle('active', p === pick));
    });
  });

  // Save / Import
  btnSave.addEventListener('click', addAccount);
  btnImport.addEventListener('click', importCurrentSession);

  // Delete confirmation
  btnConfirmCancel.addEventListener('click', () => confirmOverlay.classList.add('hidden'));
  btnConfirmDelete.addEventListener('click', deleteAccount);
  confirmOverlay.addEventListener('click', (e) => {
    if (e.target === confirmOverlay) confirmOverlay.classList.add('hidden');
  });

  // Settings toggles
  toggleAutostart.addEventListener('change', async () => {
    try {
      if (toggleAutostart.checked) {
        await enable();
        toast('SwitchCraft will start with your system');
      } else {
        await disable();
        toast('Autostart disabled');
      }
    } catch (err) {
      toast(`Autostart error: ${err}`, true);
      toggleAutostart.checked = !toggleAutostart.checked;
    }
  });

  toggleAutoupdate.addEventListener('change', saveSettings);
  toggleNotifications.addEventListener('change', saveSettings);

  // Manual update check
  btnManualUpdate.addEventListener('click', () => checkForUpdates(false));

  // Update banner
  btnUpdate.addEventListener('click', installUpdate);

  // GitHub link
  btnGithub.addEventListener('click', () => {
    if (open) open('https://github.com/dannymaaz/SwitchCraft');
  });

  // Keyboard shortcuts
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      if (!modalOverlay.classList.contains('hidden')) closeModal();
      if (!confirmOverlay.classList.contains('hidden')) confirmOverlay.classList.add('hidden');
    }
  });
}
