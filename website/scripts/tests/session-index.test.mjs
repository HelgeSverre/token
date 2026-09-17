import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync, readdirSync} from 'node:fs';
import {loadSessionData} from '../../src/lib/agent-sessions.mjs';
import {
  SESSION_PAGE_SIZE,
  buildSessionIndex,
  indexedFacetCounts,
  matchesIndexedSession,
  orderedIndexedSessions,
  paginateIndexedSessions,
} from '../../src/lib/session-index.mjs';

const readSources = dir => Object.fromEntries(readdirSync(dir).filter(name => name.startsWith('demo-') && name.endsWith('.json')).map(name => [name, readFileSync(new URL(name, dir), 'utf8')]));
const sessions = loadSessionData(
  readSources(new URL('../../src/data/agent-sessions/', import.meta.url)),
  readSources(new URL('../../src/data/agent-annotations/', import.meta.url)),
);
const index = buildSessionIndex(sessions);
const noFilters = {query:'', agents:[], bands:[], tools:[], children:false};

test('compact session index excludes transcript text and summarizes tool activity', () => {
  const viewport = index.sessions.find(session => session.id === 'demo-codex-viewport');
  assert.ok(viewport.toolCount > 0);
  assert.ok(viewport.failedToolCount > 0);
  assert.ok(viewport.subagentCount > 0);
  assert.deepEqual(Object.keys(viewport).sort(), ['agent', 'annotated', 'branch', 'displayTitle', 'failedToolCount', 'filePaths', 'id', 'parentSessionId', 'startedAt', 'subagentCount', 'toolCount', 'toolNames', 'turns']);
  assert.equal(JSON.stringify(index).includes('Keep the selection visible when wrapped lines resize'), true);
  assert.equal(JSON.stringify(index).includes('workspace restore missing directory startup'), false);
});

test('tool-name facets narrow records without reading transcript payloads', () => {
  const tests = index.sessions.filter(session => matchesIndexedSession(session, {...noFilters, tools:['test']}));
  assert.ok(tests.length > 0);
  assert.ok(tests.every(session => session.toolNames.includes('test')));
  const delegated = index.sessions.filter(session => matchesIndexedSession(session, {...noFilters, tools:['spawn_agent']}));
  assert.deepEqual(delegated.map(session => session.id), ['demo-codex-viewport']);
  const counts = indexedFacetCounts(index.sessions, {...noFilters, tools:['test']}, 'tools', ['patch', 'test']);
  assert.equal(counts.test, tests.length);
  assert.ok(counts.patch > 0);
});

test('pagination is stable, bounded, and works after chronological ordering', () => {
  const chronological = orderedIndexedSessions(index.sessions.filter(session => matchesIndexedSession(session, {...noFilters, children:true})), 'newest');
  assert.equal(chronological[0].id, 'demo-codex-split-layout');
  assert.equal(orderedIndexedSessions(chronological, 'oldest').at(-1).id, 'demo-codex-split-layout');
  const first = paginateIndexedSessions(chronological, 1, 3);
  assert.equal(first.items.length, 3);
  assert.equal(first.pages, 3);
  assert.equal(paginateIndexedSessions(chronological, 999, 3).page, 3);
  assert.equal(paginateIndexedSessions([], 1, SESSION_PAGE_SIZE).pages, 1);
});

test('chronological ordering parses ISO offsets and keeps unavailable dates last', () => {
  const records = [
    {id:'unknown', startedAt:null},
    {id:'offset', startedAt:'2026-01-01T01:00:00+01:00'},
    {id:'utc', startedAt:'2026-01-01T00:30:00Z'},
  ];
  assert.deepEqual(orderedIndexedSessions(records, 'newest').map(record => record.id), ['utc', 'offset', 'unknown']);
  assert.deepEqual(orderedIndexedSessions(records, 'oldest').map(record => record.id), ['offset', 'utc', 'unknown']);
});

test('date boundaries use inclusive UTC days and exclude missing timestamps', () => {
  const record = {...index.sessions[0], startedAt:'2026-01-03T00:30:00+01:00'};
  assert.equal(matchesIndexedSession(record, {...noFilters, children:true, from:'2026-01-02', to:'2026-01-02'}), true);
  assert.equal(matchesIndexedSession(record, {...noFilters, children:true, from:'2026-01-03'}), false);
  assert.equal(matchesIndexedSession({...record, startedAt:null}, {...noFilters, children:true, to:'2026-01-03'}), false);
});

test('file search and content facets combine with other filters', () => {
  const record = {...index.sessions[0], parentSessionId:null, filePaths:['src/Editor.rs'], annotated:true, failedToolCount:0, subagentCount:0};
  assert.equal(matchesIndexedSession(record, {...noFilters, file:'.RS', contains:['annotations']}), true);
  assert.equal(matchesIndexedSession(record, {...noFilters, file:'missing', contains:['annotations']}), false);
  assert.equal(matchesIndexedSession(record, {...noFilters, contains:['failures']}), false);
  assert.equal(matchesIndexedSession(record, {...noFilters, contains:['failures','annotations']}), true);
  assert.deepEqual(indexedFacetCounts([record], {...noFilters, contains:['failures']}, 'contains', ['annotations','failures']), {annotations:1, failures:0});
});
