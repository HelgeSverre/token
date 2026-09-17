// Build-time only. Fixtures are separate from private Agentsview snapshots.
import {createHash} from 'node:crypto';
import {z} from 'astro/zod';
import {rawTitle,turnCount} from './session-view.mjs';
import {presentSessionFiles,canonicalizeWorkspacePaths} from './session-paths.mjs';
import schema from '../../.agents/skills/annotate-session/references/session-annotation.schema.json' with {type:'json'};
const {allOf, ...structuralSchema} = schema;
// Zod's JSON Schema importer doesn't support if/then. The three evidence
// dependencies in allOf are checked below, alongside source-bound semantics.
const annotationSchema = z.fromJSONSchema(structuralSchema);
export function sourceDigest(bytes) { return createHash('sha256').update(bytes).digest('hex'); }
export function validateAnnotation(value, session, bytes) {
  const parsed = annotationSchema.safeParse(value);
  if (!parsed.success) return null;
  const a = parsed.data;
  if (a.sessionId !== session.id || a.sourceDigest !== sourceDigest(bytes)) return null;
  if (Boolean(a.title || a.summary) !== Boolean(a.evidenceMessageIds)) return null;
  const ids = new Map(session.messages.map((m,i) => [m.id || `ordinal:${m.ordinal}`,i]));
  let valid = true;
  function checkReferences(node) {
    if (!node || typeof node !== 'object') return;
    if (node.evidenceMessageIds && (new Set(node.evidenceMessageIds).size !== node.evidenceMessageIds.length || node.evidenceMessageIds.some(id=>!ids.has(id)))) valid = false;
    for (const key of ['startMessageId','endMessageId','messageId']) if (node[key] && !ids.has(node[key])) valid = false;
    Object.values(node).forEach(v=>{ if(Array.isArray(v))v.forEach(checkReferences);else if(typeof v==='object')checkReferences(v); });
  }
  checkReferences(a);
  let previous = -1;
  const chapterIds = new Set();
  for (const c of a.chapters || []) {
    const start = ids.get(c.startMessageId);
    const end = ids.get(c.endMessageId || c.startMessageId);
    if (start < previous || end < start || chapterIds.has(c.id)) valid = false;
    chapterIds.add(c.id); previous = start;
  }
  const highlightIds = (a.highlights || []).map(h=>h.id);
  if(new Set(highlightIds).size !== highlightIds.length)valid=false;
  const hasContent = a.title || a.summary || a.chapters?.length || a.highlights?.length || a.outcome || a.files?.length;
  return valid && hasContent ? a : null;
}
export function loadSessionData(sources, annotations = {}) {
  return Object.entries(sources).map(([name, bytes])=>{
    const source=JSON.parse(String(bytes));
    let annotation=null;
    const filename=name.split('/').at(-1);
    const sidecar=Object.entries(annotations).find(([key])=>key.split('/').at(-1)===filename)?.[1];
    if(sidecar) {
      try { annotation=validateAnnotation(JSON.parse(String(sidecar)),source,bytes); }
      catch { /* Optional metadata must never prevent a raw session loading. */ }
    }
    // Keep `path` and evidence exactly as supplied by the archive. Components
    // use `displayPath` for stable project-relative labels.
    return {...source,files:presentSessionFiles(source.files),annotation,displayTitle:canonicalizeWorkspacePaths(annotation?.title) || rawTitle(source),turns:turnCount(source)};
  }).sort((a,b)=>new Date(b.startedAt)-new Date(a.startedAt) || a.id.localeCompare(b.id));
}
