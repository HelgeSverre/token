import {buildContinuationIndex} from './session-continuations.mjs';
import {loadSessionData} from './agent-sessions.mjs';
import {privateArchiveRoot, readPrivateArchive} from './private-archive.mjs';
// Eager raw imports preserve the exact bytes used by annotation digests and
// let Astro/Vite track fixture edits and include sources during static builds.
const sources=import.meta.glob('../data/agent-sessions/*.json',{query:'?raw',import:'default',eager:true});
const annotations=import.meta.glob('../data/agent-annotations/*.json',{query:'?raw',import:'default',eager:true});
const previewRoot=privateArchiveRoot();
export const privatePreview=Boolean(previewRoot);
const publicSessions=loadSessionData(sources,annotations);
const recordedSessions=publicSessions.filter(session=>!session.id.startsWith('demo-'));
const previewSessions=previewRoot ? loadSessionData(readPrivateArchive(previewRoot)) : [];
// Prefer the fuller Agentsview export when a historical Amp source also exists.
const historicalId=(id:string)=>id.startsWith('amp:') ? 'T-'+id.slice(4).replace(/^T-/, '') : id;
const previewIds=new Set(previewSessions.map(session=>historicalId(session.id)));
export const sessions=previewRoot
  ? [...previewSessions,...recordedSessions.filter(session=>!previewIds.has(historicalId(session.id)))]
  : recordedSessions.length ? recordedSessions : publicSessions;
export const fictionalPreview=!privatePreview && sessions.every(session=>session.id.startsWith('demo-'));

// Relationships are derived once per catalogue load, not once per static page.
export const continuationIndex=buildContinuationIndex(sessions);
