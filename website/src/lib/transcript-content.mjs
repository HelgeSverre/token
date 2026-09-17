/**
 * A deliberately small, safe Markdown subset for imported agent prose.
 *
 * Transcript text is untrusted provider data.  This module returns structured
 * text tokens for Astro to escape, instead of turning Markdown into HTML.
 */
function text(value) {
  return typeof value === 'string' ? value.replace(/\r\n?/g, '\n') : '';
}

/**
 * Map only explicit, provider-recorded tool states to the small vocabulary the
 * transcript UI can explain. Unknown is deliberately distinct from pending:
 * Agentsview often has no result-state event at all.
 */
export function toolPresentation(status) {
  const source = text(status).trim().toLowerCase();
  if (['completed', 'complete', 'success', 'succeeded', 'passed', 'pass', 'ok', 'done', 'finished'].includes(source)) {
    return {state: 'completed', label: 'Completed', symbol: '✓'};
  }
  const failed = {
    failed: 'Failed', failure: 'Failed', error: 'Errored', errored: 'Errored',
    cancelled: 'Cancelled', canceled: 'Cancelled', aborted: 'Aborted', denied: 'Denied',
    timed_out: 'Timed out', timeout: 'Timed out', interrupted: 'Interrupted', killed: 'Killed',
  };
  if (Object.hasOwn(failed, source)) return {state: 'failed', label: failed[source], symbol: '×'};
  const pending = {pending: 'Pending', queued: 'Queued', running: 'Running', in_progress: 'In progress', 'in-progress': 'In progress', waiting: 'Waiting'};
  if (Object.hasOwn(pending, source)) return {state: 'pending', label: pending[source], symbol: '…'};
  return {state: 'unknown', label: 'Status unavailable', symbol: '·'};
}

/**
 * Claude records this exact one-line Read notation for a temporary background
 * task artifact. The archive stores the reference and the associated Read
 * call, but sampled records have no result body, so the task output itself
 * cannot be reconstructed. This parser intentionally accepts only that full
 * notation, leaving ordinary paths and prose untouched.
 */
export function claudeTaskOutputReference(value) {
  const source = text(value);
  const match = source.match(/^\s*\[Read:\s*(\/private\/tmp\/claude-[^/\s]+\/[^/\s]+\/([A-Za-z0-9-]+)\/tasks\/([A-Za-z0-9_-]+)\.output)\]\s*$/);
  if (!match) return null;
  return {path: match[1], sessionId: match[2], taskId: match[3]};
}

function xmlTag(value, name) {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = value.match(new RegExp(`<${escaped}>([\\s\\S]*?)<\\/${escaped}>`));
  return match?.[1]?.trim() || null;
}

/** Parse Claude's recorded background-task event without treating generic XML as one. */
export function claudeTaskNotification(value) {
  const source = text(value);
  if (!/^\s*<task-notification>[\s\S]*<\/task-notification>\s*$/.test(source)) return null;
  const taskId = xmlTag(source, 'task-id');
  const outputPath = xmlTag(source, 'output-file');
  if (!taskId || !outputPath) return null;
  return {
    taskId,
    outputPath,
    status: xmlTag(source, 'status'),
    summary: xmlTag(source, 'summary'),
    toolUseId: xmlTag(source, 'tool-use-id'),
    source,
  };
}

export function safeExternalHref(value) {
  if (typeof value !== 'string') return null;
  try {
    const url = new URL(value);
    return url.protocol === 'https:' || url.protocol === 'http:' ? url.href : null;
  } catch {
    return null;
  }
}

/** Split an inline line without emitting HTML. */
export function inlineTokens(value) {
  const source = text(value);
  const tokens = [];
  // Code spans, Markdown links, and the common simple emphasis forms.  The
  // unmatched text remains literal so unfamiliar Markdown is never discarded.
  const pattern = /`([^`\n]+)`|\[([^\]\n]+)\]\(([^\s)]+)\)|\*\*([^*\n]+)\*\*|__([^_\n]+)__/g;
  let cursor = 0;
  for (let match; (match = pattern.exec(source));) {
    if (match.index > cursor) tokens.push({type: 'text', value: source.slice(cursor, match.index)});
    if (match[1] !== undefined) tokens.push({type: 'code', value: match[1]});
    else if (match[2] !== undefined) {
      const href = safeExternalHref(match[3]);
      tokens.push(href ? {type: 'link', value: match[2], href} : {type: 'text', value: match[2]});
    } else tokens.push({type: 'strong', value: match[4] ?? match[5]});
    cursor = pattern.lastIndex;
  }
  if (cursor < source.length) tokens.push({type: 'text', value: source.slice(cursor)});
  return tokens;
}

function flushParagraph(blocks, lines) {
  if (lines.length) blocks.push({type: 'paragraph', value: lines.join('\n')});
  lines.length = 0;
}

function listBlock(lines, start) {
  const first = lines[start].match(/^(\s*)([-+*])\s+(.+)$/) || lines[start].match(/^(\s*)(\d+)\.\s+(.+)$/);
  if (!first) return null;
  const ordered = Boolean(first[2] && /^\d+$/.test(first[2]));
  const matcher = ordered ? /^\s*\d+\.\s+(.+)$/ : /^\s*[-+*]\s+(.+)$/;
  const items = [];
  let index = start;
  while (index < lines.length) {
    const match = lines[index].match(matcher);
    if (!match) break;
    items.push(match[1]);
    index += 1;
  }
  return {block: {type: ordered ? 'ordered-list' : 'list', items}, next: index};
}

/** Split a simple GFM table row without treating escaped pipes or code spans as separators. */
function tableCells(line) {
  const source = line.trim().replace(/^\|/, '').replace(/\|$/, '');
  const cells = [];
  let cell = '';
  let code = false;
  for (let index = 0; index < source.length; index += 1) {
    const character = source[index];
    if (character === '\\' && source[index + 1] === '|') {
      cell += '|';
      index += 1;
    } else if (character === '`') {
      code = !code;
      cell += character;
    } else if (character === '|' && !code) {
      cells.push(cell.trim());
      cell = '';
    } else cell += character;
  }
  cells.push(cell.trim());
  return cells;
}

function tableAlignment(value) {
  const marker = value.trim();
  if (!/^:?-{3,}:?$/.test(marker)) return null;
  return marker.startsWith(':') && marker.endsWith(':') ? 'center' : marker.endsWith(':') ? 'right' : 'left';
}

function tableBlock(lines, start) {
  if (start + 1 >= lines.length || !lines[start].includes('|') || !lines[start + 1].includes('|')) return null;
  const headers = tableCells(lines[start]);
  const alignments = tableCells(lines[start + 1]).map(tableAlignment);
  if (!headers.length || headers.length !== alignments.length || alignments.some(value => value === null)) return null;
  const rows = [];
  let index = start + 2;
  while (index < lines.length && lines[index].includes('|') && lines[index].trim()) {
    const cells = tableCells(lines[index]);
    // A row with extra cells is left as ordinary prose rather than dropping it.
    if (cells.length > headers.length) break;
    rows.push([...cells, ...Array(headers.length - cells.length).fill('')]);
    index += 1;
  }
  return rows.length ? {block: {type: 'table', headers, alignments, rows}, next: index} : null;
}

/**
 * Produce a compact block model for prose.  Fences are kept verbatim so tool
 * commands, diffs, and provider-formatted code never get reinterpreted.
 */
export function markdownBlocks(value) {
  const lines = text(value).split('\n');
  const blocks = [];
  const paragraph = [];
  for (let index = 0; index < lines.length;) {
    const line = lines[index];
    const fence = line.match(/^\s*(`{3,}|~{3,})\s*([^\s]*)?.*$/);
    if (fence) {
      flushParagraph(blocks, paragraph);
      const marker = fence[1];
      const language = fence[2] || '';
      const code = [];
      index += 1;
      while (index < lines.length && !new RegExp(`^\\s*${marker[0]}{${marker.length},}\\s*$`).test(lines[index])) {
        code.push(lines[index]);
        index += 1;
      }
      if (index < lines.length) index += 1;
      blocks.push({type: 'code', language, value: code.join('\n')});
      continue;
    }
    const heading = line.match(/^(#{1,6})\s+(.+)$/);
    if (heading) {
      flushParagraph(blocks, paragraph);
      blocks.push({type: 'heading', level: heading[1].length, value: heading[2]});
      index += 1;
      continue;
    }
    if (/^\s{0,3}([-*_])(?:\s*\1){2,}\s*$/.test(line)) {
      flushParagraph(blocks, paragraph);
      blocks.push({type: 'rule'});
      index += 1;
      continue;
    }
    if (line.startsWith('>')) {
      flushParagraph(blocks, paragraph);
      const quote = [];
      while (index < lines.length && lines[index].startsWith('>')) {
        quote.push(lines[index].replace(/^>\s?/, ''));
        index += 1;
      }
      blocks.push({type: 'quote', value: quote.join('\n')});
      continue;
    }
    const table = tableBlock(lines, index);
    if (table) {
      flushParagraph(blocks, paragraph);
      blocks.push(table.block);
      index = table.next;
      continue;
    }
    const list = listBlock(lines, index);
    if (list) {
      flushParagraph(blocks, paragraph);
      blocks.push(list.block);
      index = list.next;
      continue;
    }
    if (!line.trim()) {
      flushParagraph(blocks, paragraph);
      index += 1;
      continue;
    }
    paragraph.push(line);
    index += 1;
  }
  flushParagraph(blocks, paragraph);
  return blocks;
}

/** Preserve provider-embedded reasoning as an explicit, collapsible segment. */
export function messageSegments(value) {
  const source = text(value);
  const match = source.match(/^\[Thinking\]\s*\n([\s\S]*?)\n?\[\/Thinking\]\s*/);
  if (!match) return source ? [{type: 'markdown', value: source}] : [];
  const segments = [];
  if (match[1]) segments.push({type: 'thinking', value: match[1]});
  const rest = source.slice(match[0].length);
  if (rest) segments.push({type: 'markdown', value: rest});
  return segments;
}

export function publicationOmissions(session) {
  const omitted = session?.publication?.omitted;
  if (!omitted || typeof omitted !== 'object') return [];
  const labels = [
    ['unmatchedResultEvents', 'unmatched tool result event'],
    ['attachmentReferences', 'attachment reference'],
    ['dedicatedThinkingMessages', 'dedicated reasoning record'],
  ];
  return labels.flatMap(([key, label]) => {
    const count = Number(omitted[key]);
    return Number.isInteger(count) && count > 0 ? [`${count} ${label}${count === 1 ? '' : 's'}`] : [];
  });
}
