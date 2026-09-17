/* A viewer layer: no component is cloned, restyled, reparented, or reflowed. */
(() => {
  'use strict';
  const NS = 'http://www.w3.org/2000/svg';
  const clamp = (value, min, max) => Math.min(max, Math.max(min, value));
  const svgElement = (name, attributes = {}) => {
    const element = document.createElementNS(NS, name);
    for (const [key, value] of Object.entries(attributes)) element.setAttribute(key, String(value));
    return element;
  };

  function install(root, componentId) {
    const subject = window.TOKEN_MOCKUP_SUBJECTS?.[componentId];
    if (!subject?.selectors?.length) throw new Error(`Missing emphasis subject for ${componentId}.`);
    const body = root.querySelector('.specimen-body');
    const query = new URLSearchParams(location.search);
    const capturing = query.get('capture') === '1';
    let mode = query.get('view') === 'normal' ? 'normal' : 'emphasised';
    let targets = [];
    let missingSelectors = [];
    let pendingFrame = 0;
    let settings;
    const initialOverrides = new Map(['--mockup-context-opacity', '--mockup-outline-display'].map(name => [name, {
      value: root.style.getPropertyValue(name), priority: root.style.getPropertyPriority(name),
    }]));

    const style = document.createElement('style');
    style.textContent = `
      [data-mockup-emphasis] { position:absolute; pointer-events:none; z-index:2147483646; overflow:hidden; }
      .mockup-emphasis-outlines { display:var(--mockup-outline-display,none); fill:none; stroke-dasharray:3 3; }
      .mockup-view-controls { position:fixed; inset-block-start:8px; inset-inline-end:12px; z-index:2147483647; display:flex; align-items:center; gap:2px; padding:3px; border:1px solid var(--line); border-radius:4px; background:var(--desk); font:400 11px/18px var(--font-ui); }
      .mockup-view-controls > span { padding-inline:6px; color:var(--faint); }
      .mockup-view-controls button { border:1px solid transparent; border-radius:2px; min-block-size:24px; padding:2px 8px; color:var(--muted); background:transparent; font:inherit; }
      .mockup-view-controls button[aria-pressed="true"] { background:var(--control-selected); color:var(--control-selected-fg); }
      .mockup-view-controls button:focus-visible { outline:1px solid var(--focus); outline-offset:1px; }
      .mockup-context-control { display:flex; align-items:center; gap:6px; margin-inline-start:5px; padding-inline:8px; border-inline-start:1px solid var(--line); color:var(--muted); }
      .mockup-context-control input { inline-size:76px; block-size:14px; margin:0; accent-color:var(--faint); }
      .mockup-context-control output { min-inline-size:4ch; font-variant-numeric:tabular-nums; text-align:end; }
      .mockup-view-controls[data-view="normal"] .mockup-context-control { opacity:.45; }
      @media print { .mockup-view-controls { display:none; } }
    `;
    document.head.append(style);

    // One opaque-on-white mask makes overlapping subject holes a union.
    // The document-background veil composites the context once, without
    // multiplying opacity through ancestors or changing the subject's paint.
    const overlay = svgElement('svg', { 'data-mockup-emphasis': '', 'aria-hidden': 'true', focusable: 'false' });
    const maskId = 'mockup-component-emphasis-mask';
    const defs = svgElement('defs');
    const mask = svgElement('mask', { id: maskId, maskUnits: 'userSpaceOnUse', maskContentUnits: 'userSpaceOnUse', 'mask-type': 'luminance', x: 0, y: 0 });
    const background = svgElement('rect', { x: 0, y: 0, fill: 'white' });
    const holes = svgElement('g', { fill: 'black' });
    const veil = svgElement('rect', { x: 0, y: 0, mask: `url(#${maskId})` });
    const outlineMaskId = 'mockup-outline-clearance-mask';
    const outlineMask = svgElement('mask', { id: outlineMaskId, maskUnits: 'userSpaceOnUse', maskContentUnits: 'userSpaceOnUse', 'mask-type': 'luminance', x: 0, y: 0 });
    const outlineBackground = svgElement('rect', { x: 0, y: 0, fill: 'white' });
    const protectedAreas = svgElement('g', { fill: 'black' });
    const outlines = svgElement('g', { class: 'mockup-emphasis-outlines', mask: `url(#${outlineMaskId})` });
    mask.append(background, holes);
    outlineMask.append(outlineBackground, protectedAreas);
    defs.append(mask, outlineMask);
    overlay.append(defs, veil, outlines);
    document.body.append(overlay);

    let controls = null;
    if (!capturing) {
      controls = document.createElement('nav');
      controls.className = 'mockup-view-controls';
      controls.setAttribute('aria-label', 'Component reference view');
      controls.innerHTML = '<span>View</span><button type="button" data-view="normal">Normal</button><button type="button" data-view="emphasised">Emphasised</button><label class="mockup-context-control">Context<input id="mockup-context-opacity" type="range" min="0" max="100" step="5" aria-label="Context opacity"><output for="mockup-context-opacity"></output></label><button type="button" data-outline-toggle aria-pressed="false" title="Show offset inspection outlines">Outline</button>';
      controls.addEventListener('click', event => {
        const button = event.target.closest('button[data-view]');
        if (button) setMode(button.dataset.view);
        if (event.target.closest('[data-outline-toggle]')) setOutlines(!settings.outline.enabled);
      });
      controls.querySelector('input').addEventListener('input', event => setContextOpacity(Number(event.target.value) / 100));
      document.body.append(controls);
    }

    const intersects = (rect, clip, x = true, y = true) => ({
      left: x ? Math.max(rect.left, clip.left) : rect.left,
      right: x ? Math.min(rect.right, clip.right) : rect.right,
      top: y ? Math.max(rect.top, clip.top) : rect.top,
      bottom: y ? Math.min(rect.bottom, clip.bottom) : rect.bottom,
    });
    const clips = value => ['auto', 'scroll', 'hidden', 'clip'].includes(value);

    function measureTarget(element, selector, frame) {
      const elementStyle = getComputedStyle(element);
      if (elementStyle.visibility !== 'visible' || elementStyle.display === 'none') return [];
      let clip = frame;
      for (let ancestor = element.parentElement; ancestor; ancestor = ancestor.parentElement) {
        const ancestorStyle = getComputedStyle(ancestor);
        if (ancestorStyle.display === 'none' || Number(ancestorStyle.opacity) === 0) return [];
        if (ancestor === root) break;
        const clipX = clips(ancestorStyle.overflowX);
        const clipY = clips(ancestorStyle.overflowY);
        if (clipX || clipY) clip = intersects(clip, ancestor.getBoundingClientRect(), clipX, clipY);
      }
      // Keep a component's own focus outline fully lit. The optional inspection
      // guide is drawn independently, beyond this complete visible footprint.
      const outline = elementStyle.outlineStyle === 'none' ? 0 : Math.max(0, (parseFloat(elementStyle.outlineWidth) || 0) + (parseFloat(elementStyle.outlineOffset) || 0));
      return [...element.getClientRects()].flatMap(rect => {
        if (rect.width <= 0 || rect.height <= 0) return [];
        const visible = intersects({ left: rect.left - outline, top: rect.top - outline, right: rect.right + outline, bottom: rect.bottom + outline }, clip);
        const width = visible.right - visible.left;
        const height = visible.bottom - visible.top;
        if (width <= 0 || height <= 0) return [];
        const radiusValue = elementStyle.borderTopLeftRadius;
        const radius = radiusValue.endsWith('%') ? Math.min(rect.width, rect.height) * parseFloat(radiusValue) / 100 : parseFloat(radiusValue) || 0;
        return [{ selector, x: visible.left - frame.left, y: visible.top - frame.top, width, height, radius: Math.min(Math.max(0, radius + outline), width / 2, height / 2) }];
      });
    }

    function readSettings() {
      const style = getComputedStyle(root);
      const number = (name, fallback, min, max) => {
        const value = Number.parseFloat(style.getPropertyValue(name));
        return Number.isFinite(value) ? clamp(value, min, max) : fallback;
      };
      outlines.style.setProperty('--mockup-outline-display', style.getPropertyValue('--mockup-outline-display').trim() || 'none');
      return {
        contextOpacity: number('--mockup-context-opacity', 0.25, 0, 1),
        outline: {
          enabled: getComputedStyle(outlines).display !== 'none',
          offset: number('--mockup-outline-offset', 8, 4, 48),
          width: number('--mockup-outline-width', 1, 0.5, 3),
          color: style.getPropertyValue('--mockup-outline-color').trim() || style.getPropertyValue('--faint').trim(),
        },
      };
    }

    function targetRect({ x, y, width, height, radius }, expansion = 0) {
      return svgElement('rect', { x: x - expansion, y: y - expansion, width: width + expansion * 2, height: height + expansion * 2, rx: radius + expansion, ry: radius + expansion });
    }

    function refresh() {
      if (pendingFrame) cancelAnimationFrame(pendingFrame);
      pendingFrame = 0;
      settings = readSettings();
      const frame = root.getBoundingClientRect();
      targets = [];
      missingSelectors = [];
      for (const selector of subject.selectors) {
        const matches = [...body.querySelectorAll(selector)].flatMap(element => measureTarget(element, selector, frame));
        if (!matches.length) missingSelectors.push(selector);
        targets.push(...matches);
      }
      overlay.style.left = `${frame.left + scrollX}px`;
      overlay.style.top = `${frame.top + scrollY}px`;
      overlay.setAttribute('width', frame.width);
      overlay.setAttribute('height', frame.height);
      overlay.setAttribute('viewBox', `0 0 ${frame.width} ${frame.height}`);
      for (const element of [mask, background, veil, outlineMask, outlineBackground]) {
        element.setAttribute('width', frame.width);
        element.setAttribute('height', frame.height);
      }
      veil.setAttribute('fill', getComputedStyle(root).backgroundColor);
      veil.setAttribute('fill-opacity', 1 - settings.contextOpacity);
      holes.replaceChildren(...targets.map(target => targetRect(target)));
      // Protect every subject plus its clearance, including adjacent/overlapping
      // subjects. A guide can never cross another subject's pixels or safe gap.
      protectedAreas.replaceChildren(...targets.map(target => targetRect(target, settings.outline.offset)));
      outlines.setAttribute('stroke', settings.outline.color);
      outlines.setAttribute('stroke-width', settings.outline.width);
      outlines.replaceChildren(...targets.map(target => targetRect(target, settings.outline.offset + settings.outline.width / 2)));
      // A broken mapping fails validation but keeps the browser page legible.
      overlay.style.display = mode === 'emphasised' && targets.length && !missingSelectors.length ? 'block' : 'none';
      root.dataset.referenceView = mode;
      if (controls) {
        controls.dataset.view = mode;
        for (const button of controls.querySelectorAll('button[data-view]')) button.setAttribute('aria-pressed', String(button.dataset.view === mode));
        const input = controls.querySelector('input');
        const percentage = Math.round(settings.contextOpacity * 100);
        input.value = percentage;
        input.disabled = mode === 'normal';
        input.setAttribute('aria-valuetext', `${percentage}% context opacity`);
        controls.querySelector('output').textContent = `${percentage}%`;
        const outlineButton = controls.querySelector('[data-outline-toggle]');
        outlineButton.setAttribute('aria-pressed', String(settings.outline.enabled));
        outlineButton.disabled = mode === 'normal';
      }
      return inspect();
    }

    function inspect() {
      return { componentId, mode, contextOpacity: settings.contextOpacity, outline: { ...settings.outline }, description: subject.description, targets: targets.map(target => ({ ...target })), missingSelectors: [...missingSelectors] };
    }

    function updateQuery(name, value) {
      const url = new URL(location.href);
      url.searchParams.set(name, value);
      history.replaceState(null, '', url);
    }

    function setMode(value, { updateUrl = true } = {}) {
      if (!['normal', 'emphasised'].includes(value)) throw new Error(`Unknown mockup view: ${value}`);
      mode = value;
      if (updateUrl) updateQuery('view', mode);
      return refresh();
    }

    function setContextOpacity(value, { updateUrl = true } = {}) {
      if (typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value > 1) throw new Error('Context opacity must be between 0 and 1.');
      root.style.setProperty('--mockup-context-opacity', String(value));
      if (updateUrl) updateQuery('context', String(Math.round(value * 100)));
      return refresh();
    }

    function setOutlines(value, { updateUrl = true } = {}) {
      if (typeof value !== 'boolean') throw new Error('Outline visibility must be a boolean.');
      root.style.setProperty('--mockup-outline-display', value ? 'block' : 'none');
      if (updateUrl) updateQuery('outlines', value ? '1' : '0');
      return refresh();
    }

    function applyLocationSettings() {
      for (const [name, { value, priority }] of initialOverrides) {
        if (value) root.style.setProperty(name, value, priority);
        else root.style.removeProperty(name);
      }
      const params = new URLSearchParams(location.search);
      const opacity = params.get('context');
      if (opacity?.trim() && Number.isFinite(Number(opacity))) root.style.setProperty('--mockup-context-opacity', String(clamp(Number(opacity) / 100, 0, 1)));
      if (['0', '1'].includes(params.get('outlines'))) root.style.setProperty('--mockup-outline-display', params.get('outlines') === '1' ? 'block' : 'none');
      mode = params.get('view') === 'normal' ? 'normal' : 'emphasised';
      return refresh();
    }

    const scheduleRefresh = () => {
      if (!pendingFrame) pendingFrame = requestAnimationFrame(refresh);
    };
    new ResizeObserver(scheduleRefresh).observe(root);
    new MutationObserver(scheduleRefresh).observe(body, { subtree: true, childList: true, attributes: true, characterData: true });
    new MutationObserver(scheduleRefresh).observe(root, { attributes: true, attributeFilter: ['class', 'style'] });
    root.addEventListener('scroll', scheduleRefresh, true);
    window.addEventListener('resize', scheduleRefresh);
    window.addEventListener('popstate', applyLocationSettings);
    window.MockupEmphasis = Object.freeze({ setMode, setContextOpacity, setOutlines, refresh, inspect });
    applyLocationSettings();
  }

  window.installMockupEmphasis = install;
})();
