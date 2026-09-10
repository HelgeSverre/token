// Code fences remain escaped text in the HTML. Mermaid receives textContent,
// never source interpolated into executable JavaScript or HTML.
if (typeof hljs !== 'undefined') {
    document.querySelectorAll('pre code:not(.language-mermaid)').forEach((block) => {
        hljs.highlightElement(block);
    });
}

function diagramError(pre, message) {
    const error = document.createElement('p');
    error.className = 'mermaid-error';
    error.setAttribute('role', 'status');
    error.textContent = message;
    pre.after(error);
}

async function renderMermaidDiagrams() {
    const blocks = Array.from(document.querySelectorAll('pre > code.language-mermaid'));
    if (blocks.length === 0) return;

    const mermaid = window.mermaid;
    try {
        const page = getComputedStyle(document.body);
        const code = getComputedStyle(blocks[0].parentElement);
        mermaid.initialize({
            startOnLoad: false,
            securityLevel: 'strict',
            suppressErrorRendering: true,
            theme: 'base',
            themeVariables: {
                fontFamily: page.fontFamily,
                background: page.backgroundColor,
                primaryColor: code.backgroundColor,
                primaryBorderColor: page.color,
                primaryTextColor: page.color,
                textColor: page.color,
                lineColor: page.color,
                actorLineColor: page.color,
                edgeLabelBackground: page.backgroundColor,
            },
        });
    } catch (_) {
        blocks.forEach((code) => diagramError(code.parentElement,
            'The bundled Mermaid renderer could not be initialized; the diagram source is left visible.'));
        return;
    }

    for (const code of blocks) {
        const pre = code.parentElement;
        const diagram = document.createElement('div');
        diagram.className = 'mermaid-diagram';
        diagram.style.visibility = 'hidden';
        diagram.textContent = code.textContent;
        pre.after(diagram);
        try {
            await mermaid.run({ nodes: [diagram] });
            pre.remove();
            diagram.style.removeProperty('visibility');
        } catch (_) {
            diagram.remove();
            diagramError(pre, 'Mermaid diagram could not be rendered. Check its syntax; the source is left visible.');
        }
    }
}

renderMermaidDiagrams();

// Scroll to a specific source line. The source markers stay in place when a
// code block becomes a diagram.
window.scrollToLine = function(line) {
    const el = document.querySelector(`[data-line="${line}"]`);
    if (el) el.scrollIntoView({ behavior: 'smooth', block: 'start' });
};

let scrollTimeout = null;
window.addEventListener('scroll', function() {
    if (scrollTimeout) clearTimeout(scrollTimeout);
    scrollTimeout = setTimeout(function() {
        const elements = document.querySelectorAll('[data-line]');
        let visibleLine = null;
        for (const el of elements) {
            if (el.getBoundingClientRect().top >= 0) {
                visibleLine = parseInt(el.getAttribute('data-line'), 10);
                break;
            }
        }
        if (visibleLine !== null && window.webkit && window.webkit.messageHandlers) {
            window.webkit.messageHandlers.scrollSync.postMessage({ line: visibleLine });
        }
    }, 100);
});
