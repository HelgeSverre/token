/* Static visual-reference helpers. The source of all theme colors is dark.yaml. */
(() => {
  'use strict';
  const supportBase = new URL('.', document.currentScript.src);
  let emphasisSupport;
  function loadEmphasisSupport() {
    emphasisSupport ??= Promise.all(['subjects.js', 'emphasis.js'].map(filename => new Promise((resolve, reject) => {
      const script = document.createElement('script');
      script.src = new URL(filename, supportBase).href;
      script.onload = resolve;
      script.onerror = () => reject(new Error(`Could not load mockup support: ${filename}`));
      document.head.append(script);
    })));
    return emphasisSupport;
  }
  const theme = window.TOKEN_PERFORMANCE_THEMES?.themes.find(({ id }) => id === 'default-dark');
  if (!theme) throw new Error('Load debug-performance-themes.js before shared.js.');
  for (const [name, color] of Object.entries(theme.colors)) {
    document.documentElement.style.setProperty(`--${name.replace(/^--/, '')}`, color);
  }
  document.documentElement.dataset.theme = theme.id;

  const escape = (value) => String(value ?? '').replace(/[&<>"']/g, (character) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character]);
  const shapes = {
    search: '<circle cx="7" cy="7" r="4.5"/><path d="m10.4 10.4 3.5 3.5"/>',
    close: '<path d="m4 4 8 8M12 4l-8 8"/>',
    plus: '<path d="M8 3v10M3 8h10"/>',
    minus: '<path d="M3 8h10"/>',
    check: '<path d="m3 8 3.2 3.2L13 4.5"/>',
    'chevron-right': '<path d="m6 3 5 5-5 5"/>',
    'chevron-left': '<path d="m10 3-5 5 5 5"/>',
    'chevron-down': '<path d="m3 5 5 5 5-5"/>',
    'chevron-up': '<path d="m3 10 5-5 5 5"/>',
    'arrow-right': '<path d="M2 8h11M8 3l5 5-5 5"/>',
    'arrow-left': '<path d="M14 8H3m5-5L3 8l5 5"/>',
    'arrow-up': '<path d="M8 14V3m-5 5 5-5 5 5"/>',
    'arrow-down': '<path d="M8 2v11m-5-5 5 5 5-5"/>',
    folder: '<path d="M2 4h4l1.5 1.5H14v7H2z"/>',
    file: '<path d="M4 1.8h5l3 3v9.4H4zM9 2v3h3"/>',
    code: '<path d="m5 4-4 4 4 4m6-8 4 4-4 4M9.5 2l-3 12"/>',
    terminal: '<rect x="1.5" y="2.5" width="13" height="11" rx="1"/><path d="m4 5 3 3-3 3m5 0h3"/>',
    settings: '<path d="m6.3 1.8-.5 1.8-1.7.9-1.8-.4-1 1.8 1.2 1.4v1.9l-1.2 1.4 1 1.8 1.8-.4 1.7.9.5 1.8h2l.5-1.8 1.7-.9 1.8.4 1-1.8-1.2-1.4V7.3l1.2-1.4-1-1.8-1.8.4-1.7-.9-.5-1.8z"/><circle cx="7.3" cy="8" r="2.3"/>',
    more: '<circle cx="3" cy="8" r=".6"/><circle cx="8" cy="8" r=".6"/><circle cx="13" cy="8" r=".6"/>',
    info: '<circle cx="8" cy="8" r="6"/><path d="M8 7v4M8 4.5v.5"/>',
    warning: '<path d="M8 2 15 14H1zM8 6v4M8 11.5v.5"/>',
    error: '<circle cx="8" cy="8" r="6"/><path d="m5.5 5.5 5 5m0-5-5 5"/>',
    refresh: '<path d="M13.5 6a5.5 5.5 0 1 0-.8 5M13.5 2v4h-4"/>',
    play: '<path d="m5 2 9 6-9 6z"/>',
    pause: '<path d="M5 3v10M11 3v10"/>',
    dock: '<rect x="1.5" y="2" width="13" height="12" rx="1"/><path d="M10 2v12M10 5h4"/>',
    float: '<rect x="2" y="5" width="9" height="9" rx="1"/><path d="M6 2h8v8M8 8l6-6"/>',
    split: '<rect x="1.5" y="2.5" width="13" height="11" rx="1"/><path d="M8 2.5v11"/>',
    chart: '<path d="M2 2v12h12M4 10l3-4 3 2 4-5"/>',
    copy: '<rect x="5" y="5" width="8" height="9" rx="1"/><path d="M10 5V2H2v9h3"/>',
    link: '<path d="m6.2 10.2 3.6-4.4M6 5l1.7-1.8a3 3 0 0 1 4.3 4.2l-1.5 1.8M10 11l-1.7 1.8a3 3 0 0 1-4.3-4.2l1.5-1.8"/>',
    pin: '<path d="m6 2 6 6-2 1-2.5-.5L6 11 5 10l2.5-1.5L7 6 6 4zM5.5 10.5 2 14"/>',
    eye: '<path d="M1 8s2.5-5 7-5 7 5 7 5-2.5 5-7 5-7-5-7-5Z"/><circle cx="8" cy="8" r="2"/>',
    'git-branch': '<circle cx="4" cy="3" r="1.6"/><circle cx="4" cy="13" r="1.6"/><circle cx="12" cy="4" r="1.6"/><path d="M4 5v6m0-2c6 0 8-1 8-3"/>',
    keyboard: '<rect x="1" y="3" width="14" height="10" rx="1"/><path d="M4 6h.1M7 6h.1M10 6h.1M12 6h.1M4 9h.1M7 9h.1M10 9h.1M5 11h6"/>',
    external: '<path d="M9 2h5v5M14 2 7 9M6 3H2v11h11v-4"/>',
    save: '<path d="M2 2h10l2 2v10H2zM5 2v5h6V2M5 14v-4h6v4"/>',
    trash: '<path d="M2 4h12M5 4V2h6v2M4 4l1 10h6l1-10M7 7v4M9 7v4"/>',
    clock: '<circle cx="8" cy="8" r="6"/><path d="M8 4v4l3 2"/>',
    filter: '<path d="M2 3h12L9 9v4l-2 1V9z"/>',
  };
  function icon(name, size = 16) {
    const aliases = { x: 'close', chevron: 'chevron-right', branch: 'git-branch', 'external-link': 'external', 'arrow-up-right': 'external', 'chevrons-up-down': 'chevron-down', 'file-code': 'file', sliders: 'settings', 'panel-right': 'dock', maximize: 'float', 'more-horizontal': 'more', 'check-circle': 'check', 'alert-triangle': 'warning', 'circle-alert': 'info', 'rotate-ccw': 'refresh', 'chevrons-right': 'chevron-right' };
    const shape = shapes[aliases[name] ?? name];
    if (!shape) throw new Error(`Unknown mockup icon: ${name}`);
    return `<svg class="icon" width="${Number(size)}" height="${Number(size)}" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.35" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${shape}</svg>`;
  }
  const defaultLines = [
    '/// Layout shared by rendering and hit testing.',
    'pub fn editor_chrome(',
    '    bounds: Rect,',
    '    metrics: &UiMetrics,',
    '    show_breadcrumbs: bool,',
    ') -> EditorChrome {',
    '    let tabs = metrics.tab_height();',
    '    let breadcrumb = if show_breadcrumbs {',
    '        metrics.context_bar_height()',
    '    } else {',
    '        0.0',
    '    };',
    '',
    '    let content_top = bounds.y + tabs + breadcrumb;',
    '    let viewport = Rect::new(',
    '        bounds.x, content_top,',
    '        bounds.width, bounds.bottom() - content_top,',
    '    );',
    '    EditorChrome { tabs, breadcrumb, viewport }',
    '}',
  ];
  function highlight(line) {
    const pattern = /(\/\/.*$)|("(?:[^"\\]|\\.)*")|\b(pub|fn|let|mut|if|else|return|struct|impl|use|match|Some|None|true|false|self|for|in|const)\b|\b([A-Z][A-Za-z0-9_]*)\b|\b([a-z_][a-z_0-9]*)(?=\s*\()|\b(\d+(?:\.\d+)?)\b/g;
    let result = '', end = 0;
    for (const match of String(line).matchAll(pattern)) {
      result += escape(line.slice(end, match.index));
      const kind = match[1] ? 'comment' : match[2] ? 'string' : match[3] ? 'keyword' : match[4] ? 'type' : match[5] ? 'function' : 'number';
      result += `<span class="syntax-${kind}">${escape(match[0])}</span>`;
      end = match.index + match[0].length;
    }
    return result + escape(line.slice(end));
  }
  function code({ lines = defaultLines, start = 114, active = 132 } = {}) {
    const rows = typeof lines === 'string' ? lines.split('\n') : lines;
    return `<div class="code-view" data-clip-demo aria-label="Rust source code">${rows.map((line, i) => {
      const number = typeof line === 'object' && line.number != null ? line.number : start + i;
      const value = typeof line === 'object' ? line.text ?? line.code ?? '' : String(line);
      const selected = number === active || line?.active === true;
      return `<div class="code-line${selected ? ' active' : ''}"><span class="code-number">${escape(number)}</span><span class="code-source">${highlight(value)}</span></div>`;
    }).join('')}<span class="static-scrollbar" aria-hidden="true"></span></div>`;
  }
  function tree({ selected = 'chrome.rs' } = {}) {
    return [
      ['token-editor', 0, true, true], ['src', 1, true, true], ['layout', 2, true, true],
      ['chrome.rs', 2, false], ['editor.rs', 2, false], ['mod.rs', 2, false],
      ['view', 1, true, false], ['model', 1, true, false], ['themes', 0, true, false],
      ['Cargo.toml', 0, false], ['README.md', 0, false],
    ].map(([label, depth, folder, open]) => `<div class="tree-row depth-${depth}${label === selected ? ' selected' : ''}">${folder ? icon(open ? 'chevron-down' : 'chevron-right', 11) : '<span style="inline-size:11px;flex:none"></span>'}<span class="${folder ? 'folder' : 'file'}">${icon(folder ? 'folder' : 'file', 14)}</span><span>${escape(label)}</span></div>`).join('');
  }
  function editor({ title = 'chrome.rs', tabs = [title, 'perf.rs'], breadcrumb = false, sidebar = false, status = false, footer = '', lines } = {}) {
    const tabLabel = (tab) => typeof tab === 'string' ? tab : tab.title ?? tab.label ?? tab.name;
    const currentIndex = Math.max(0, tabs.findIndex((tab) => tabLabel(tab) === title));
    const location = typeof breadcrumb === 'object' ? breadcrumb : { path: ['src', title] };
    const segments = [...(location.path ?? ['src', title]), ...(location.symbol ? [location.symbol] : [])];
    const breadcrumbHtml = segments.map((segment, index) => `<span${index === segments.length - 1 ? ' class="current"' : ''}>${escape(segment)}</span>`).join(icon('chevron-right', 10));
    const activeIndex = Array.isArray(lines) ? lines.findIndex((line) => line?.active === true) : -1;
    const cursorLine = activeIndex >= 0 ? lines[activeIndex].number ?? 114 + activeIndex : 132;
    return `<article class="window editor-context" aria-label="Token editor context"><div class="editor-tabs">${tabs.map((tab, i) => {
      const label = tabLabel(tab);
      const active = typeof tab === 'object' && tab.active !== undefined ? tab.active : i === currentIndex;
      return `<div class="editor-tab${active ? ' active' : ''}">${icon('file', 14)}<span>${escape(label)}</span><span class="close">${typeof tab === 'object' && tab.modified ? '•' : icon('close', 11)}</span></div>`;
    }).join('')}<span class="grow"></span></div><div class="editor-middle">${sidebar ? `<aside class="editor-sidebar"><div class="tree-heading">Explorer${icon('more')}</div>${tree({ selected: title })}</aside>` : ''}<section class="editor-document">${breadcrumb ? `<nav class="breadcrumb" aria-label="Document location">${breadcrumbHtml}</nav>` : ''}${code({ lines, active: cursorLine })}${footer ? `<div class="surface-footer">${footer}</div>` : ''}</section></div>${status ? `<footer class="status-bar"><span class="row">${icon('git-branch', 12)} main</span><span class="row">${icon('error', 12)} 0 ${icon('warning', 12)} 2</span><span class="grow"></span><span>Ln ${escape(cursorLine)}, Col 5</span><span>Spaces: 4</span><span>UTF-8</span><span>Rust</span></footer>` : ''}</article>`;
  }
  function sparkline({ values = [12, 11, 13, 16, 15, 11, 9, 12, 18, 14, 12, 13, 11, 14, 16, 14, 12], color = 'var(--blue)', width = 240, height = 70 } = {}) {
    const max = Math.max(...values, 1) * 1.18;
    const points = values.map((value, i) => `${(i * width / Math.max(values.length - 1, 1)).toFixed(2)},${(height - 5 - value / max * (height - 10)).toFixed(2)}`);
    return `<svg class="sparkline" width="${Number(width)}" height="${Number(height)}" viewBox="0 0 ${Number(width)} ${Number(height)}" role="img" aria-label="Fixed illustrative performance history"><path d="M0 ${height * .3}H${width}M0 ${height * .65}H${width}" stroke="var(--chart-grid)" stroke-width="1"/><path d="M0 ${height}L${points.join('L')}L${width} ${height}Z" fill="${escape(color)}" opacity=".07"/><polyline points="${points.join(' ')}" fill="none" stroke="${escape(color)}" stroke-width="1.5" stroke-linejoin="round"/></svg>`;
  }
  async function mount({ id, title, description, status = 'Established surface · visual study', content }) {
    if (document.querySelector('#specimen')) throw new Error('A specimen is already mounted.');
    document.title = `${title} · Token UI reference`;
    document.body.insertAdjacentHTML('beforeend', `<main id="specimen" data-component="${escape(id)}"><header class="specimen-header"><div class="specimen-heading"><div class="specimen-kicker">Token / component reference</div><h1>${escape(title)}</h1><p class="specimen-description">${escape(description)}</p></div><span class="specimen-status">${escape(status)}</span></header><section class="specimen-body">${content}</section><footer class="specimen-footer"><span>Default Dark · Inter / JetBrains Mono · 1200 × 760</span><span>Static visual reference · ${escape(id)}</span></footer></main>`);
    await Promise.all([document.fonts.load('13px Inter'), document.fonts.load('12.5px "JetBrains Mono"')]);
    await document.fonts.ready;
    await loadEmphasisSupport();
    window.installMockupEmphasis(document.querySelector('#specimen'), id);
    window.__mockupReady = true;
  }
  window.Mockup = Object.freeze({ icon, code, tree, editor, sparkline, mount, escape });
})();
