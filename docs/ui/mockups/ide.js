/* Optional fixed desktop surroundings for compact component specimens. */
(() => {
  'use strict';
  const { escape, icon } = Mockup;
  function preferences({ section = 'Editor', title = section, content, footer = '', categories = ['Appearance', 'Editor', 'Files', 'Language servers', 'Key bindings', 'About Token'] }) {
    return `<section class="ide-window ide-context" aria-label="Token Settings">
      <header class="ide-titlebar"><span>Settings</span><span class="grow"></span><span class="ide-path">Token</span></header>
      <div class="ide-preferences"><nav class="ide-categories" aria-label="Settings categories">
        <div class="ide-filter">${icon('search', 13)}<span>Search settings</span></div>
        ${categories.map(label => `<div class="ide-category"${label === section ? ' aria-current="page"' : ''}>${escape(label)}</div>`).join('')}
      </nav><section class="ide-settings-content"><h2 class="ide-settings-heading">${escape(title)}</h2>${content}</section></div>
      ${footer ? `<footer class="ide-footer">${footer}</footer>` : ''}
    </section>`;
  }
  window.Ide = Object.freeze({ preferences });
})();
