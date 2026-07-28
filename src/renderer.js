const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const tabsEl = document.getElementById('tabs');
const addressEl = document.getElementById('address');
const starBtn = document.getElementById('star-btn');
const panelOverlay = document.getElementById('panel-overlay');
const panel = document.getElementById('panel');
const panelTitle = document.getElementById('panel-title');
const panelBody = document.getElementById('panel-body');

let currentUrl = '';
let currentBookmarks = [];

document.getElementById('back').addEventListener('click', () => invoke('go_back'));
document.getElementById('forward').addEventListener('click', () => invoke('go_forward'));
document.getElementById('reload').addEventListener('click', () => invoke('reload'));
document.getElementById('new-tab').addEventListener('click', () => invoke('new_tab', { url: null }));
document.getElementById('history-btn').addEventListener('click', () => openHistoryPanel());
document.getElementById('bookmarks-btn').addEventListener('click', () => openBookmarksPanel());
starBtn.addEventListener('click', async () => {
  const active = getActiveTitle();
  await invoke('toggle_bookmark', { url: currentUrl, title: active });
  refreshBookmarkStar();
});

addressEl.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') invoke('navigate', { url: addressEl.value });
});

document.addEventListener('keydown', (e) => {
  const cmd = e.ctrlKey || e.metaKey;
  if (cmd && e.key.toLowerCase() === 't') { invoke('new_tab', { url: null }); e.preventDefault(); }
  else if (cmd && e.key.toLowerCase() === 'l') { addressEl.focus(); addressEl.select(); e.preventDefault(); }
  else if (cmd && e.key.toLowerCase() === 'd') { starBtn.click(); e.preventDefault(); }
});

let lastTabs = [];

listen('tabs-updated', ({ payload }) => {
  lastTabs = payload.tabs;
  renderTabs(payload.tabs, payload.activeId);
});

function renderTabs(tabs, activeId) {
  tabsEl.innerHTML = '';
  tabs.forEach(t => {
    const el = document.createElement('div');
    el.className = 'tab' + (t.id === activeId ? ' active' : '');
    el.innerHTML = `<span class="globe">&#127760;</span><span class="title">${escapeHtml(t.title)}</span><span class="close">&times;</span>`;
    el.addEventListener('click', (ev) => {
      if (ev.target.classList.contains('close')) return;
      invoke('switch_tab', { id: t.id });
    });
    el.querySelector('.close').addEventListener('click', (ev) => {
      ev.stopPropagation();
      invoke('close_tab', { id: t.id });
    });
    tabsEl.appendChild(el);
  });

  const active = tabs.find(t => t.id === activeId);
  if (active) {
    currentUrl = active.url;
    if (document.activeElement !== addressEl) addressEl.value = active.url;
    refreshBookmarkStar();
  }
}

function getActiveTitle() {
  const t = lastTabs.find(t => t.url === currentUrl);
  return t ? t.title : currentUrl;
}

function refreshBookmarkStar() {
  starBtn.classList.toggle('active', currentBookmarks.some(b => b.url === currentUrl));
}

// --- Panels ---
function closePanel() {
  panel.classList.add('hidden');
  panelOverlay.classList.add('hidden');
}
document.getElementById('panel-close').addEventListener('click', closePanel);
panelOverlay.addEventListener('click', closePanel);

function openPanel(title) {
  panelTitle.textContent = title;
  panel.classList.remove('hidden');
  panelOverlay.classList.remove('hidden');
}

async function openHistoryPanel() {
  openPanel('History');
  const list = await invoke('get_history');
  panelBody.innerHTML = '';
  if (list.length === 0) panelBody.innerHTML = '<div class="panel-empty">No history yet</div>';
  [...list].reverse().slice(0, 300).forEach(item => {
    const row = document.createElement('div');
    row.className = 'panel-row';
    row.innerHTML = `<div class="main">${escapeHtml(item.title)}<div class="sub">${escapeHtml(item.url)}</div></div>`;
    row.querySelector('.main').addEventListener('click', () => { invoke('navigate', { url: item.url }); closePanel(); });
    panelBody.appendChild(row);
  });
  if (list.length) {
    const footer = document.createElement('div');
    footer.className = 'panel-footer';
    footer.innerHTML = '<button id="clear-history-btn">Clear history</button>';
    panelBody.appendChild(footer);
    document.getElementById('clear-history-btn').addEventListener('click', async () => {
      await invoke('clear_history');
      openHistoryPanel();
    });
  }
}

async function openBookmarksPanel() {
  openPanel('Bookmarks');
  currentBookmarks = await invoke('get_bookmarks');
  renderBookmarksPanel(currentBookmarks);
}
function renderBookmarksPanel(list) {
  panelBody.innerHTML = '';
  if (list.length === 0) panelBody.innerHTML = '<div class="panel-empty">No bookmarks yet — click the star to add one</div>';
  list.forEach(b => {
    const row = document.createElement('div');
    row.className = 'panel-row';
    row.innerHTML = `<div class="main">${escapeHtml(b.title)}<div class="sub">${escapeHtml(b.url)}</div></div><button class="remove">&times;</button>`;
    row.querySelector('.main').addEventListener('click', () => { invoke('navigate', { url: b.url }); closePanel(); });
    row.querySelector('.remove').addEventListener('click', async () => {
      await invoke('remove_bookmark', { url: b.url });
      currentBookmarks = await invoke('get_bookmarks');
      renderBookmarksPanel(currentBookmarks);
    });
    panelBody.appendChild(row);
  });
}

function escapeHtml(str) {
  const div = document.createElement('div');
  div.textContent = str || '';
  return div.innerHTML;
}

invoke('get_bookmarks').then(list => { currentBookmarks = list; });
