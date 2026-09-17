import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync,readdirSync} from 'node:fs';
import {loadSessionData,validateAnnotation,sourceDigest} from '../../src/lib/agent-sessions.mjs';
import {matchesSession,facetCounts,groupMessages,relativeAge,rawTitle,turnCount,agentName,fullTimestamp,absoluteDate} from '../../src/lib/session-view.mjs';
const readSources=(dir)=>Object.fromEntries(readdirSync(dir).filter(n=>n.startsWith('demo-')&&n.endsWith('.json')).map(n=>[n,readFileSync(new URL(n,dir),'utf8')]));
const sessions=loadSessionData(readSources(new URL('../../src/data/agent-sessions/',import.meta.url)),readSources(new URL('../../src/data/agent-annotations/',import.meta.url)));
const none={query:'',agents:[],bands:[],children:false};
const bytes=readFileSync(new URL('../../src/data/agent-sessions/demo-codex-viewport.json',import.meta.url));
const source=JSON.parse(bytes);
const editorial=JSON.parse(readFileSync(new URL('../../src/data/agent-annotations/demo-codex-viewport.json',import.meta.url)));

test('main browser is chronological, hides child runs, and counts real human messages',()=>{
 assert.equal(sessions.filter(s=>matchesSession(s,none)).length,7);
 assert.equal(sessions.filter(s=>matchesSession(s,{...none,children:true})).length,8);
 assert.equal(turnCount(source),24);
 assert.ok(sessions.every((s,i)=>i===0||Date.parse(sessions[i-1].startedAt)>=Date.parse(s.startedAt)));
});
test('facets combine OR within agent and AND with turn band/title',()=>{
 const filters={...none,agents:['codex','claude'],bands:['long'],query:'selection'};
 assert.deepEqual(sessions.filter(s=>matchesSession(s,filters)).map(s=>s.id),['demo-codex-viewport']);
 assert.equal(sessions.filter(s=>matchesSession(s,{...filters,agents:['amp']})).length,0);
});
test('disjunctive facet counts retain other facets and title search',()=>{
 const counts=facetCounts(sessions,{...none,agents:['amp'],bands:['long']},'agent',['amp','codex','claude']);
 assert.deepEqual(counts,{amp:0,codex:1,claude:0});
 assert.equal(facetCounts(sessions,{...none,query:'zz-no-match'},'turns',['short']).short,0);
});
test('raw viewer does not require an editorial title or annotations',()=>{
 const untitled=sessions.find(s=>!s.title);
 assert.ok(untitled);assert.equal(untitled.annotation,null);
 assert.equal(untitled.displayTitle,rawTitle(untitled));
 assert.equal(sessions.find(s=>s.id==='demo-claude-settings-audit').annotation,null);
});
test('message grouping preserves provider changes, system boundaries, and consecutive human turns',()=>{
 const groups=groupMessages({agent:'codex',messages:[{id:'a',role:'user'},{id:'b',role:'user'},{id:'c',role:'assistant'},{id:'d',role:'assistant'},{id:'e',role:'assistant',agent:'claude'},{id:'f',role:'system'}]});
 assert.deepEqual(groups.map(g=>g.messages.length),[1,1,2,1,1]);
 assert.equal(agentName(groups[3].agent),'Claude');
});
test('relative age handles past, future, and missing dates',()=>{
 const now=Date.parse('2026-09-16T00:00:00Z');
 assert.equal(relativeAge('2026-09-12T00:00:00Z',now),'4 days ago');
 assert.equal(relativeAge('2026-09-17T00:00:00Z',now),'in 1 day');
 assert.equal(relativeAge('bad',now),'');
});
test('sidecars are bound to exact source bytes and ignored when stale or mismatched',()=>{
 assert.ok(validateAnnotation(editorial,source,bytes));
 assert.equal(validateAnnotation(editorial,source,Buffer.concat([bytes,Buffer.from(' ')])),null);
 assert.equal(validateAnnotation({...editorial,sessionId:'other'},source,bytes),null);
 assert.equal(validateAnnotation({...editorial,unknown:'field'},source,bytes),null);
});
test('annotation evidence and ordered chapters are required to resolve to source messages',()=>{
 const base={schemaVersion:1,sessionId:source.id,sourceDigest:sourceDigest(bytes)};
 assert.equal(validateAnnotation(base,source,bytes),null);
 assert.equal(validateAnnotation({...base,title:'Uncited'},source,bytes),null);
 assert.equal(validateAnnotation({...base,title:'Missing',evidenceMessageIds:['not-a-message']},source,bytes),null);
 assert.equal(validateAnnotation({...base,title:'Duplicate',evidenceMessageIds:['m001','m001']},source,bytes),null);
 assert.equal(validateAnnotation({...editorial,chapters:[...editorial.chapters].reverse()},source,bytes),null);
});
test('fixture file stats and subagent links have source evidence; unannotated files omit invented counts',()=>{
 for(const s of sessions){const ids=new Set(s.messages.map(m=>m.id));assert.equal(ids.size,s.messages.length);for(const f of s.files)assert.ok(f.evidenceMessageIds.every(id=>ids.has(id)));for(const m of s.messages)for(const t of m.tools||[])if(t.subagentSessionId)assert.ok(sessions.some(other=>other.id===t.subagentSessionId));}
 const raw=sessions.find(s=>s.id==='demo-claude-settings-audit');
 assert.ok(raw.files.length>=18);assert.ok(raw.files.some(f=>f.added===undefined));assert.ok(source.files.length>=25);
});

test('timestamp tooltip is complete, UTC-normalized, and honest about missing dates',()=>{
 assert.equal(fullTimestamp('2026-09-16T12:34:56+02:00'),'2026-09-16 10:34:56 UTC');
 assert.equal(fullTimestamp('2026-09-16T12:34:56.123Z'),'2026-09-16 12:34:56 UTC');
 assert.equal(fullTimestamp(null),'Timestamp unavailable');
 assert.equal(absoluteDate(null),'Date unavailable');
 assert.equal(relativeAge(null),'');
 assert.equal(fullTimestamp('invalid'),'Timestamp unavailable');
});

test('historical records without file metadata retain a usable raw view', () => {
 const [session]=loadSessionData({'historical.json':JSON.stringify({id:'T-history',agent:'amp',startedAt:null,messages:[{id:'m1',role:'user',text:'A recorded prompt'}]})});
 assert.deepEqual(session.files,[]);
 assert.equal(session.turns,1);
 assert.equal(session.displayTitle,'A recorded prompt');
 assert.equal(session.annotation,null);
});
