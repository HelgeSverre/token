/**
 * The browser catalogue deliberately contains metadata only. Full transcript
 * payloads stay on their own static session pages, so filtering a large archive
 * never transfers message bodies or tool output to the listing page.
 */
export const SESSION_INDEX_VERSION = 2;
export const SESSION_PAGE_SIZE = 25;

export const TURN_BANDS = [
  {id:'short', label:'1–4 turns'},
  {id:'medium', label:'5–19 turns'},
  {id:'long', label:'20+ turns'},
];

export const CONTAINS_OPTIONS = [
  {id:'annotations', label:'Annotations'},
  {id:'failures', label:'Failed commands'},
  {id:'delegation', label:'Delegated work'},
];

export function containsFeature(record, key) {
  return key === 'annotations' ? record.annotated
    : key === 'failures' ? record.failedToolCount > 0
      : key === 'delegation' ? record.subagentCount > 0 : false;
}

export function indexTurnBand(turns) {
  return turns < 5 ? 'short' : turns < 20 ? 'medium' : 'long';
}

function timestampValue(record) {
  const value = Date.parse(String(record.startedAt || ''));
  return Number.isFinite(value) ? value : null;
}

function compareByStart(left, right, sort = 'newest') {
  const leftTime = timestampValue(left);
  const rightTime = timestampValue(right);
  // Unknown dates always follow dated sessions. Reversing the sort must never
  // make an unavailable timestamp look older than a known session.
  if (leftTime === null || rightTime === null) {
    if (leftTime === rightTime) return left.id.localeCompare(right.id);
    return leftTime === null ? 1 : -1;
  }
  const comparison = leftTime - rightTime;
  return comparison === 0 ? left.id.localeCompare(right.id) : comparison * (sort === 'oldest' ? 1 : -1);
}

export function toolSummary(messages = []) {
  const tools = messages.flatMap(message => message.tools || []);
  return {
    toolCount: tools.length,
    failedToolCount: tools.filter(tool => tool.status === 'failed').length,
    subagentCount: tools.filter(tool => Boolean(tool.subagentSessionId)).length,
    toolNames: [...new Set(tools.map(tool => String(tool.name || '').trim()).filter(Boolean))].sort(),
  };
}

export function compactSession(session) {
  return {
    id: session.id,
    displayTitle: session.displayTitle,
    agent: session.agent || '',
    turns: session.turns || 0,
    parentSessionId: session.parentSessionId || null,
    startedAt: session.startedAt || null,
    branch: session.branch || null,
    annotated: Boolean(session.annotation),
    // The browser index is presentation metadata. Use the same canonical label
    // as Files touched, while the full session retains provider paths/evidence.
    filePaths: [...new Set((session.files || []).map(file => file.displayPath || file.path).filter(Boolean))].sort(),
    ...toolSummary(session.messages),
  };
}

function countBy(records, predicate) {
  return records.reduce((count, record) => count + Number(predicate(record)), 0);
}

export function buildSessionIndex(sessions) {
  const records = sessions.map(compactSession).sort((left, right) => compareByStart(left, right));
  const main = records.filter(record => !record.parentSessionId);
  const agents = [...new Set(records.map(record => record.agent).filter(Boolean))].sort();
  const toolNames = [...new Set(records.flatMap(record => record.toolNames))].sort();
  const summary = records => ({
    sessions: records.length,
    toolCalls: records.reduce((total, record) => total + record.toolCount, 0),
    failedToolCalls: records.reduce((total, record) => total + record.failedToolCount, 0),
    delegatedTasks: records.reduce((total, record) => total + record.subagentCount, 0),
  });
  return {
    version: SESSION_INDEX_VERSION,
    pageSize: SESSION_PAGE_SIZE,
    sessions: records,
    facets: {
      contains: CONTAINS_OPTIONS.map(({id}) => ({id, count: countBy(main, record => containsFeature(record, id))})),
      agents: agents.map(id => ({id, count: countBy(main, record => record.agent === id)})),
      turns: TURN_BANDS.map(({id}) => ({id, count: countBy(main, record => indexTurnBand(record.turns) === id)})),
      tools: toolNames.map(id => ({id, count: countBy(main, record => record.toolNames.includes(id))}))
        .sort((left, right) => right.count - left.count || left.id.localeCompare(right.id)),
    },
    totals: {all: summary(records), main: summary(main)},
  };
}

export function matchesIndexedSession(record, filters, omit = '') {
  const title = String(record.displayTitle || '').toLocaleLowerCase();
  const started = timestampValue(record);
  const from = filters.from ? Date.parse(filters.from + 'T00:00:00Z') : null;
  const through = filters.to ? Date.parse(filters.to + 'T00:00:00Z') + 86400000 : null;
  return (!filters.from || (started !== null && started >= from))
    && (!filters.to || (started !== null && started < through))
    && (!filters.file || (record.filePaths || []).some(path => path.toLocaleLowerCase().includes(filters.file.trim().toLocaleLowerCase())))
    && (omit === 'contains' || !(filters.contains || []).length || filters.contains.some(key => containsFeature(record, key)))
    && (filters.children || !record.parentSessionId)
    && title.includes(String(filters.query || '').trim().toLocaleLowerCase())
    && (omit === 'agent' || !filters.agents.length || filters.agents.includes(record.agent))
    && (omit === 'turns' || !filters.bands.length || filters.bands.includes(indexTurnBand(record.turns)))
    && (omit === 'tools' || !filters.tools.length || filters.tools.some(tool => record.toolNames.includes(tool)));
}

export function indexedFacetCounts(records, filters, facet, keys) {
  return Object.fromEntries(keys.map(key => [key, records.filter(record =>
    matchesIndexedSession(record, filters, facet) && (
      facet === 'agent' ? record.agent === key
        : facet === 'turns' ? indexTurnBand(record.turns) === key
          : facet === 'contains' ? containsFeature(record, key) : record.toolNames.includes(key)
    ),
  ).length]));
}

export function orderedIndexedSessions(records, sort = 'newest') {
  return [...records].sort((left, right) => compareByStart(left, right, sort));
}

export function paginateIndexedSessions(records, page, pageSize = SESSION_PAGE_SIZE) {
  const pages = Math.max(1, Math.ceil(records.length / pageSize));
  const currentPage = Math.max(1, Math.min(Number.parseInt(String(page), 10) || 1, pages));
  const start = (currentPage - 1) * pageSize;
  return {items: records.slice(start, start + pageSize), page: currentPage, pages, total: records.length};
}
