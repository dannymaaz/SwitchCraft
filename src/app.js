/* ============================================
   SwitchCraft — Frontend Application Logic
   ============================================ */

function getTauri() {
  return window.__TAURI__ || {};
}

const invoke = (...args) => {
  const tauri = getTauri();
  if (tauri.core && tauri.core.invoke) {
    return tauri.core.invoke(...args);
  }
  console.warn('[SwitchCraft] Tauri core.invoke not ready yet');
  return Promise.resolve();
};

const check = () => getTauri().updater?.check?.();
const relaunch = () => getTauri().process?.relaunch?.();
const enableAutostart = () => getTauri().autostart?.enable?.();
const disableAutostart = () => getTauri().autostart?.disable?.();
const isAutostartEnabled = () => getTauri().autostart?.isEnabled?.() || Promise.resolve(false);

// ── State ──────────────────────────────────────
let accounts = [];
let selectedPlatform = 'codex';
let selectedAuthTab = 'key';
let deleteTargetId = null;
let showPassword = false;
let settings = {
  autoUpdate: true,
  notifications: true,
};

const PLATFORM_CONFIG = {
  codex: {
    name: 'Codex / OpenAI',
    keyLabel: 'OpenAI API Key / Token',
    keyPlaceholder: 'sk-proj-... or personal API token',
    keyHint: 'Used for Codex and OpenAI developer tools.',
    browserTitle: 'Sign in to OpenAI / Codex',
    browserDesc: 'Open OpenAI or ChatGPT in your browser to sign in, then capture the session below.',
    loginUrl: 'https://platform.openai.com/api-keys',
  },
  gemini: {
    name: 'Gemini / Antigravity',
    keyLabel: 'Google AI Studio Key / Token',
    keyPlaceholder: 'AIzaSy... or OAuth session token',
    keyHint: 'Works seamlessly with Google Antigravity and Gemini CLI.',
    browserTitle: 'Sign in to Google AI / Antigravity',
    browserDesc: 'Open Google AI Studio to authenticate or generate an API key, then capture session.',
    loginUrl: 'https://aistudio.google.com/app/apikey',
  },
  claude: {
    name: 'Claude / Anthropic',
    keyLabel: 'Anthropic API Key / Session Key',
    keyPlaceholder: 'sk-ant-... or session token',
    keyHint: 'Works with Claude Desktop and Anthropic developer tools.',
    browserTitle: 'Sign in to Claude / Anthropic',
    browserDesc: 'Open Claude Console or Claude.ai to sign in, then capture the session.',
    loginUrl: 'https://console.anthropic.com/settings/keys',
  },
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
const inputApiKey = $('#input-api-key');
const inputCreds = $('#input-creds');
const labelApiKey = $('#label-api-key');
const hintApiKey = $('#hint-api-key');
const btnToggleKeyVisibility = $('#btn-toggle-key-visibility');
const browserLoginTitle = $('#browser-login-title');
const browserLoginDesc = $('#browser-login-desc');
const btnLaunchBrowserLogin = $('#btn-launch-browser-login');
const btnImportBrowser = $('#btn-import-browser');
const authTabs = $$('.auth-tab');
const authTabContents = $$('.auth-tab-content');

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
async function initApp() {
  await loadVersion();
  await loadAccounts();
  await loadSettings();
  setupEventListeners();

  // Check for updates on startup (if enabled)
  if (settings.autoUpdate) {
    setTimeout(() => checkForUpdates(true), 2000);
  }
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', initApp);
} else {
  initApp();
}

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

        const platformIcons = {
          codex: '<svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M22.28 9.82a5.98 5.98 0 0 0-.51-4.91 6.05 6.05 0 0 0-6.51-2.9A6.07 6.07 0 0 0 4.98 4.18a5.98 5.98 0 0 0-4 2.9 6.05 6.05 0 0 0 .74 7.1 5.98 5.98 0 0 0 .51 4.91 6.05 6.05 0 0 0 6.51 2.9A5.98 5.98 0 0 0 13.26 24a6.06 6.06 0 0 0 5.77-4.21 5.99 5.99 0 0 0 4-2.9 6.06 6.06 0 0 0-.75-7.07zm-9.02 12.61a4.48 4.48 0 0 1-2.88-1.04l.14-.08 4.78-2.76a.79.79 0 0 0 .39-.68v-6.74l2.02 1.17a.07.07 0 0 1 .04.05v5.58a4.5 4.5 0 0 1-4.49 4.5zm-9.66-5.58a4.47 4.47 0 0 1-.53-3l.14.08 4.78 2.76a.77.77 0 0 0 .78 0l5.85-3.37v2.33a.08.08 0 0 1-.04.06L9.74 19.95a4.5 4.5 0 0 1-6.14-3.1zm-1.44-9.68a4.48 4.48 0 0 1 2.34-1.95v5.61a.79.79 0 0 0 .4.68l5.84 3.37-2.02 1.17a.07.07 0 0 1-.07 0L3.8 12.08a4.5 4.5 0 0 1-1.64-4.91zm15.11 4.67-5.84-3.37 2.02-1.17a.07.07 0 0 1 .07 0l4.83 2.8a4.5 4.5 0 0 1-.67 8.1v-5.68a.79.79 0 0 0-.41-.68zm2.01-3.02-.14-.09-4.78-2.78a.78.78 0 0 0-.78 0L9.41 6.39V4.06a.08.08 0 0 1 .03-.06l4.88-2.83a4.5 4.5 0 0 1 6.35 4.91zm-9.84-3.42a4.48 4.48 0 0 1 2.87 1.04l-.14.08-4.78 2.76a.79.79 0 0 0-.39.68v6.74l-2.02-1.17a.07.07 0 0 1-.04-.05V8.08a4.5 4.5 0 0 1 4.49-4.49zm1.1 5.88 2.71 1.57v3.13l-2.7 1.57-2.72-1.57V11.46l2.71-1.56z"/></svg>',
          gemini: '<svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M12 0C12 6.627 6.627 12 0 12c6.627 0 12 5.373 12 12 0-6.627 5.373-12 12-12-6.627 0-12-5.373-12-12z"/></svg>',
          claude: '<svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M4.5 10.5C3.67 10.5 3 11.17 3 12s.67 1.5 1.5 1.5h1.76l-1.24 1.24c-.59.59-.59 1.54 0 2.12.59.59 1.54.59 2.12 0l1.24-1.24V17.5c0 .83.67 1.5 1.5 1.5s1.5-.67 1.5-1.5v-1.88l1.24 1.24c.59.59 1.54.59 2.12 0 .59-.59.59-1.54 0-2.12l-1.24-1.24H19.5c.83 0 1.5-.67 1.5-1.5s-.67-1.5-1.5-1.5h-1.88l1.24-1.24c.59-.59.59-1.54 0-2.12-.59-.59-1.54-.59-2.12 0L15.5 8.12V6.5c0-.83-.67-1.5-1.5-1.5s-1.5.67-1.5 1.5v1.76l-1.24-1.24c-.59-.59-1.54-.59-2.12 0-.59.59-.59 1.54 0 2.12l1.24 1.24H4.5z"/></svg>',
        };
        const iconSvg = platformIcons[account.platform] || initial;

        return `
          <div class="account-card ${isActive ? 'active' : ''}" data-id="${account.id}" title="Click to switch to ${escapeHtml(account.name)}">
            <div class="account-avatar account-avatar--${account.platform}">${iconSvg}</div>
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
              <button class="action-btn action-btn--delete" data-delete="${account.id}" title="Remove profile" aria-label="Remove ${escapeHtml(account.name)}">
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
              </button>
            </div>
          </div>`;
      })
      .join('');
  });

  // Toggle empty state vs platform sections
  platformSections.classList.toggle('hidden', totalCount === 0);
  emptyState.classList.toggle('hidden', totalCount > 0);

  // Attach card click handlers for switching
  document.querySelectorAll('.account-card[data-id]').forEach((card) => {
    card.addEventListener('click', (e) => {
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

function updateModalForPlatform() {
  const cfg = PLATFORM_CONFIG[selectedPlatform] || PLATFORM_CONFIG.codex;
  labelApiKey.textContent = cfg.keyLabel;
  inputApiKey.placeholder = cfg.keyPlaceholder;
  hintApiKey.textContent = cfg.keyHint;
  browserLoginTitle.textContent = cfg.browserTitle;
  browserLoginDesc.textContent = cfg.browserDesc;
}

function selectAuthTab(tab) {
  selectedAuthTab = tab;
  authTabs.forEach((t) => t.classList.toggle('active', t.dataset.tab === tab));
  authTabContents.forEach((c) => c.classList.toggle('active', c.id === `tab-content-${tab}`));
}

async function addAccount() {
  const name = inputName.value.trim();

  if (!name) {
    toast('Enter a profile name', true);
    inputName.focus();
    return;
  }

  let credentials;

  if (selectedAuthTab === 'key') {
    const key = inputApiKey.value.trim();
    if (!key) {
      toast('Enter an API key or session token', true);
      inputApiKey.focus();
      return;
    }

    if (selectedPlatform === 'codex') {
      credentials = {
        apiKey: key,
        auth_mode: 'api_key',
        created_at: new Date().toISOString(),
      };
    } else if (selectedPlatform === 'gemini') {
      credentials = {
        apiKey: key,
        auth_mode: 'api_key',
        created_at: new Date().toISOString(),
      };
    } else if (selectedPlatform === 'claude') {
      credentials = {
        apiKey: key,
        sessionKey: key,
        auth_mode: 'api_key',
        created_at: new Date().toISOString(),
      };
    }
  } else if (selectedAuthTab === 'json') {
    const credsRaw = inputCreds.value.trim();
    if (!credsRaw) {
      toast('Paste your credentials JSON', true);
      inputCreds.focus();
      return;
    }
    try {
      credentials = JSON.parse(credsRaw);
    } catch {
      toast('Invalid JSON — check your format', true);
      return;
    }
  } else if (selectedAuthTab === 'browser') {
    await importCurrentSession();
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
    inputName.focus();
    return;
  }

  try {
    await invoke('import_current_account', {
      name,
      platform: selectedPlatform,
    });

    closeModal();
    toast(`Imported active ${selectedPlatform} session as "${name}"`);
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
    const autoEnabled = await isAutostartEnabled();
    toggleAutostart.checked = autoEnabled;
  } catch {
    toggleAutostart.checked = false;
  }

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
      if (!silent) toast("You're on the latest version");
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
      if (relaunch) await relaunch();
    }
  } catch (err) {
    toast(`Update install failed: ${err}`, true);
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
  inputApiKey.value = '';
  inputCreds.value = '';
  inputPlan.value = '';
  inputWeeklyLimit.value = '';
  inputHourlyLimit.value = '';
  selectedPlatform = 'codex';
  showPassword = false;
  inputApiKey.type = 'password';
  btnToggleKeyVisibility.textContent = '👁️ Show';

  platformPicks.forEach((p) => p.classList.toggle('active', p.dataset.pick === 'codex'));
  selectAuthTab('key');
  updateModalForPlatform();

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
    await invoke('minimize_window');
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

  // Auth tabs
  authTabs.forEach((tab) => {
    tab.addEventListener('click', () => selectAuthTab(tab.dataset.tab));
  });

  // Toggle key visibility
  btnToggleKeyVisibility.addEventListener('click', () => {
    showPassword = !showPassword;
    inputApiKey.type = showPassword ? 'text' : 'password';
    btnToggleKeyVisibility.textContent = showPassword ? '🙈 Hide' : '👁️ Show';
  });

  // Launch official web login
  btnLaunchBrowserLogin.addEventListener('click', async () => {
    const cfg = PLATFORM_CONFIG[selectedPlatform] || PLATFORM_CONFIG.codex;
    try {
      await invoke('open_browser_url', { url: cfg.loginUrl });
      toast(`Opening ${cfg.name} login...`);
    } catch (err) {
      toast(`Could not open browser: ${err}`, true);
    }
  });

  // Capture session from browser tab button
  btnImportBrowser.addEventListener('click', importCurrentSession);

  // Platform picker
  platformPicks.forEach((pick) => {
    pick.addEventListener('click', () => {
      selectedPlatform = pick.dataset.pick;
      platformPicks.forEach((p) => p.classList.toggle('active', p === pick));
      updateModalForPlatform();
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
        await enableAutostart();
        toast('SwitchCraft will start with your system');
      } else {
        await disableAutostart();
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
  btnGithub.addEventListener('click', async () => {
    try {
      await invoke('open_browser_url', { url: 'https://github.com/dannymaaz/SwitchCraft' });
    } catch {
      window.open('https://github.com/dannymaaz/SwitchCraft', '_blank');
    }
  });

  // Keyboard shortcuts
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      if (!modalOverlay.classList.contains('hidden')) closeModal();
      if (!confirmOverlay.classList.contains('hidden')) confirmOverlay.classList.add('hidden');
    }
  });
}
