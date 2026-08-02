// The 8 tally palettes (from desktop/src-ui/themes.js). The ◐ button in the
// header cycles them; choice persists in localStorage. Loaded in <head> so the
// saved theme applies before first paint.

const PALETTES = [
  { name: 'Catppuccin', bg: '#1e1e2e', surface: '#181825', overlay: '#313244', text: '#cdd6f4', subtext: '#a6adc8', blue: '#89b4fa', green: '#a6e3a1', mauve: '#cba6f7', red: '#f38ba8', yellow: '#f9e2af', teal: '#94e2d5', border: '#45475a', statusBg: '#11111b' },
  { name: 'White on Black', bg: '#000000', surface: '#0a0a0a', overlay: '#1a1a1a', text: '#ffffff', subtext: '#999999', blue: '#6fa8dc', green: '#8fbc8f', mauve: '#b8a9c9', red: '#d98a8a', yellow: '#d4c476', teal: '#7fbfbf', border: '#333333', statusBg: '#000000' },
  { name: 'Black on White', bg: '#ffffff', surface: '#f5f5f5', overlay: '#e8e8e8', text: '#000000', subtext: '#666666', blue: '#2563eb', green: '#16a34a', mauve: '#7c3aed', red: '#dc2626', yellow: '#ca8a04', teal: '#0d9488', border: '#cccccc', statusBg: '#f0f0f0' },
  { name: 'Rose Pine', bg: '#191724', surface: '#1f1d2e', overlay: '#26233a', text: '#e0def4', subtext: '#908caa', blue: '#9ccfd8', green: '#31748f', mauve: '#c4a7e7', red: '#eb6f92', yellow: '#f6c177', teal: '#9ccfd8', border: '#2a2837', statusBg: '#16141f' },
  { name: 'Tokyo Night', bg: '#1a1b26', surface: '#16161e', overlay: '#292e42', text: '#c0caf5', subtext: '#787c99', blue: '#7aa2f7', green: '#9ece6a', mauve: '#bb9af7', red: '#f7768e', yellow: '#e0af68', teal: '#73daca', border: '#3b4261', statusBg: '#13131e' },
  { name: 'Soft Ember', bg: '#1c1917', surface: '#181412', overlay: '#2c2622', text: '#e7ddd5', subtext: '#a8998e', blue: '#e8a87c', green: '#a3be8c', mauve: '#d4a0c0', red: '#cf8989', yellow: '#e8c47c', teal: '#8fbcbb', border: '#3d3530', statusBg: '#141110' },
  { name: 'Nord', bg: '#2e3440', surface: '#272c36', overlay: '#3b4252', text: '#d8dee9', subtext: '#939aad', blue: '#88c0d0', green: '#a3be8c', mauve: '#b48ead', red: '#bf616a', yellow: '#ebcb8b', teal: '#8fbcbb', border: '#4c566a', statusBg: '#242933' },
  { name: 'Moonlight', bg: '#1e2030', surface: '#191b28', overlay: '#2f334d', text: '#c8d3f5', subtext: '#828bb8', blue: '#82aaff', green: '#c3e88d', mauve: '#c099ff', red: '#ff757f', yellow: '#ffc777', teal: '#86e1fc', border: '#3b3f5c', statusBg: '#161825' },
];

function applyPalette(p) {
  const root = document.documentElement.style;
  root.setProperty('--bg', p.bg);
  root.setProperty('--surface', p.surface);
  root.setProperty('--overlay', p.overlay);
  root.setProperty('--text', p.text);
  root.setProperty('--subtext', p.subtext);
  root.setProperty('--blue', p.blue);
  root.setProperty('--green', p.green);
  root.setProperty('--mauve', p.mauve);
  root.setProperty('--red', p.red);
  root.setProperty('--yellow', p.yellow);
  root.setProperty('--teal', p.teal);
  root.setProperty('--border', p.border);
  root.setProperty('--status-bg', p.statusBg);
  const meta = document.querySelector('meta[name="theme-color"]');
  if (meta) meta.content = p.statusBg;
}

function currentThemeIndex() {
  const i = parseInt(localStorage.getItem('tally-theme') || '0', 10);
  return Number.isFinite(i) && i >= 0 && i < PALETTES.length ? i : 0;
}

applyPalette(PALETTES[currentThemeIndex()]);

window.cycleTheme = function () {
  const next = (currentThemeIndex() + 1) % PALETTES.length;
  localStorage.setItem('tally-theme', String(next));
  applyPalette(PALETTES[next]);
  const note = document.getElementById('status-note');
  if (note) {
    const prev = note.textContent;
    note.textContent = PALETTES[next].name.toLowerCase();
    setTimeout(() => { note.textContent = prev; }, 1200);
  }
};
