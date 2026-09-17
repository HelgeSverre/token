/** Display helpers shared by the static renderer and small browser enhancements. */
export {agentName} from './agent-identity.mjs';
import {canonicalizeWorkspacePaths} from './session-paths.mjs';
/** Detail headings retain the complete source; the browser can use a compact title. */
export function detailTitle(session) {
  const prompt = session.messages.find(m => m.role === 'user' && m.text?.trim())?.text.trim();
  if (session.titleSource === 'first_human_turn' && prompt) return canonicalizeWorkspacePaths(prompt);
  return canonicalizeWorkspacePaths(session.title?.trim() || prompt || session.id);
}
export function rawTitle(session) {
  if (session.title?.trim()) return canonicalizeWorkspacePaths(session.title.trim());
  const prompt = canonicalizeWorkspacePaths(session.messages.find(m => m.role === 'user' && m.text?.trim())?.text.trim().split('\n')[0]);
  if (!prompt) return session.id;
  return prompt.length > 140 ? prompt.slice(0, 139).replace(/\s+\S*$/, '').trimEnd() + '…' : prompt;
}
export function turnCount(session) { return session.messages.filter(m => m.role === 'user').length; }
export function turnBand(turns) { return turns < 5 ? 'short' : turns < 20 ? 'medium' : 'long'; }
export function absoluteDate(iso) {
  if (!iso) return 'Date unavailable';
  const date = new Date(iso);
  return Number.isNaN(date.valueOf()) ? 'Date unavailable' : new Intl.DateTimeFormat('en-GB',{day:'numeric',month:'short',year:'numeric',timeZone:'UTC'}).format(date);
}
/** Unambiguous second-precision UTC timestamp for native hover tooltips. */
export function fullTimestamp(iso) {
  if (!iso) return 'Timestamp unavailable';
  const date = new Date(iso);
  return Number.isNaN(date.valueOf()) ? 'Timestamp unavailable' : date.toISOString().slice(0,19).replace('T',' ') + ' UTC';
}
export function relativeAge(iso, now = Date.now()) {
  if (!iso) return '';
  const seconds = (new Date(iso).valueOf() - now) / 1000;
  if (!Number.isFinite(seconds)) return '';
  for (const [unit, size] of [['year',31536000],['month',2592000],['day',86400],['hour',3600],['minute',60]]) {
    if (Math.abs(seconds) >= size) return new Intl.RelativeTimeFormat('en',{numeric:'always'}).format(Math.trunc(seconds / size), unit);
  }
  return 'just now';
}
export function matchesSession(session, filters, omit = '') {
  const title = (session.displayTitle || rawTitle(session)).toLocaleLowerCase();
  return (filters.children || !session.parentSessionId)
    && title.includes(filters.query.trim().toLocaleLowerCase())
    && (omit === 'agent' || !filters.agents.length || filters.agents.includes(session.agent))
    && (omit === 'turns' || !filters.bands.length || filters.bands.includes(turnBand(session.turns ?? turnCount(session))));
}
export function facetCounts(sessions, filters, facet, keys) {
  return Object.fromEntries(keys.map(key => [key, sessions.filter(s => matchesSession(s, filters, facet) && (facet === 'agent' ? s.agent === key : turnBand(s.turns ?? turnCount(s)) === key)).length]));
}
export function groupMessages(session) {
  const groups = [];
  for (const message of session.messages) {
    const provider = message.agent || session.agent;
    const key = message.role + ':' + provider;
    const last = groups.at(-1);
    // Preserve every human turn, even if two prompts arrive consecutively.
    if (message.role === 'assistant' && last?.key === key) last.messages.push(message);
    else groups.push({key,role:message.role,agent:provider,messages:[message]});
  }
  return groups;
}
