import {absoluteDate,relativeAge,fullTimestamp} from './session-view.mjs';
import {transcriptChunkPath} from './transcript-chunks.mjs';
const frame=document.querySelector<HTMLElement>('[data-session-detail]')!;
const root=frame.dataset.rootId!;
const views: Array<{id:string;title:string;agent:string;branch?:string;commits:Array<{sha:string;evidenceMessageId:string}>;startedAt:string;turns:number;messages:number;parent?:string;chunks:number}>=JSON.parse(frame.dataset.views!);
type ChunkMetadata={size:number;count:number;messagePages:Record<string,number>};
const chunks: Record<string,ChunkMetadata>=JSON.parse(frame.dataset.transcriptChunks!);
const editor=frame.querySelector<HTMLElement>('.session-editor')!;
const context=frame.querySelector<HTMLElement>('.session-context')!;
const panes=new Map<string,HTMLElement>([[root,frame.querySelector<HTMLElement>('[data-view]')!]]);
const loadedChunks=new Map<string,number>([[root,1]]);
const chunkRequests=new Map<string,Promise<boolean>>();
const positions=new Map<string,number>();
const openers=new Map<string,HTMLElement>();
const breadcrumb=frame.querySelector<HTMLElement>('.session-breadcrumb')!;
const back=breadcrumb.querySelector<HTMLAnchorElement>('[data-parent-back]')!;
const outline=frame.querySelector<HTMLElement>('[data-outline-label]')!;
const nav=frame.querySelector<HTMLDetailsElement>('.navigation-disclosure')!;
const toggle=frame.querySelector<HTMLButtonElement>('#annotation-toggle');
const title=frame.querySelector<HTMLElement>('[data-session-title]')!;
let active=root;
let navigationGeneration=0;
const mobile=matchMedia('(max-width: 768px)');
function syncSections(){nav.open=!mobile.matches;context.querySelectorAll<HTMLDetailsElement>('details').forEach(el=>el.open=!mobile.matches);}
syncSections();mobile.addEventListener('change',syncSections);
const raw=frame.querySelector<HTMLElement>('[data-raw-navigation]')!;
function outlineLabel(){outline.textContent=active!==root?'Parent outline':!raw.hidden?'Human turns':'Chapters';}
function setAnnotations(on:boolean){
  frame.dataset.annotations=on?'on':'off';
  frame.querySelectorAll<HTMLElement>('[data-editorial]').forEach(el=>el.hidden=!on);
  raw.hidden=on&&!!frame.querySelector('[aria-label="Annotated chapters"]');
  title.textContent=on&&title.dataset.editorialTitle?title.dataset.editorialTitle:title.dataset.sourceTitle!;
  toggle?.setAttribute('aria-pressed',String(on));if(toggle)toggle.textContent=on?'Hide annotations':'Show annotations';
  outlineLabel();
}
if(toggle){toggle.hidden=false;if(new URLSearchParams(location.search).get('annotations')==='off')setAnnotations(false);toggle.addEventListener('click',()=>{setAnnotations(frame.dataset.annotations!=='on');const url=new URL(location.href);if(frame.dataset.annotations==='on')url.searchParams.delete('annotations');else url.searchParams.set('annotations','off');history.replaceState(history.state,'',url);});}
function paneFor(id:string){
  if(panes.has(id))return panes.get(id)!;
  const template=[...frame.querySelectorAll<HTMLTemplateElement>('[data-child-template]')].find(t=>t.dataset.childTemplate===id);
  if(!template)return null;
  const fragment=template.content.cloneNode(true) as DocumentFragment;
  const pane=fragment.querySelector<HTMLElement>('[data-view]')!;
  pane.hidden=true;editor.append(fragment);panes.set(id,pane);loadedChunks.set(id,0);return pane;
}
function rebasedChildChunk(fragment:DocumentFragment,id:string){
  for(const element of fragment.querySelectorAll<HTMLElement>('[id^="message-"]'))element.id=`${id}-${element.id}`;
}
function loadMoreLabel(id:string,next:number){
  const data=views.find(view=>view.id===id)!;
  return `Load next ${Math.min(chunks[id].size,Math.max(0,data.messages-(next*chunks[id].size)))} messages`;
}
async function appendChunk(id:string,page?:number){
  const metadata=chunks[id];
  const next=loadedChunks.get(id) ?? 0;
  if(!metadata||next>=metadata.count)return true;
  if(page!==undefined&&page!==next)return page<next;
  const existing=chunkRequests.get(id);if(existing)return existing;
  const pane=paneFor(id);if(!pane)return false;
  const link=pane.querySelector<HTMLAnchorElement>('[data-load-more]');
  if(link){link.setAttribute('aria-busy','true');link.textContent=next===0?'Loading transcript…':loadMoreLabel(id,next);}
  const request=(async()=>{try{
    const response=await fetch(transcriptChunkPath(id,next));
    if(!response.ok)throw new Error(`Transcript chunk ${next} failed to load`);
    const template=document.createElement('template');template.innerHTML=await response.text();
    const chunk=template.content.querySelector<HTMLElement>('[data-transcript-chunk]');
    const messages=pane.querySelector<HTMLElement>('[data-transcript-messages]');
    if(!chunk||!messages)throw new Error('Transcript chunk was invalid');
    const fragment=document.createDocumentFragment();
    while(chunk.firstChild)fragment.append(chunk.firstChild);
    if(id!==root)rebasedChildChunk(fragment,id);
    if(frame.dataset.annotations==='off')fragment.querySelectorAll<HTMLElement>('[data-editorial]').forEach(element=>element.hidden=true);
    messages.append(fragment);
    loadedChunks.set(id,next+1);
    if(link){
      if(next+1>=metadata.count)link.remove();
      else{link.dataset.nextChunk=String(next+1);link.href=transcriptChunkPath(id,next+1);link.textContent=loadMoreLabel(id,next+1);link.removeAttribute('aria-busy');}
    }
    return true;
  }catch(error){
    if(link){link.removeAttribute('aria-busy');link.textContent=next===0?'Retry transcript':`Retry ${loadMoreLabel(id,next).toLocaleLowerCase()}`;}
    console.warn(error);
    return false;
  }})();
  chunkRequests.set(id,request);
  void request.finally(()=>{if(chunkRequests.get(id)===request)chunkRequests.delete(id);});
  return request;
}
async function ensureChunk(id:string,page:number){
  while((loadedChunks.get(id)??0)<=page){
    if(!await appendChunk(id))return false;
  }
  return true;
}
function sourceMessageId(anchor:string,id:string){
  if(anchor.startsWith('message-'))return anchor.slice('message-'.length);
  const prefix=`${id}-message-`;
  return anchor.startsWith(prefix)?anchor.slice(prefix.length):null;
}
function anchorLocation(anchor:string,preferred=root){
  const candidates=[preferred,active,root,...views.map(view=>view.id)].filter((id,index,all)=>all.indexOf(id)===index);
  for(const id of candidates){
    const messageId=sourceMessageId(anchor,id);
    const page=messageId==null?undefined:chunks[id]?.messagePages[messageId];
    if(page!==undefined)return {id,page,target:id===root?`message-${messageId}`:`${id}-message-${messageId}`,messageId};
  }
  return null;
}
function show(id:string,focus=false){
  const data=views.find(v=>v.id===id),pane=paneFor(id);if(!data||!pane)return false;
  navigationGeneration++;
  positions.set(active,panes.get(active)!.scrollTop);
  active=id;
  for(const [key,el] of panes){el.hidden=key!==id;el.inert=key!==id;}
  const child=id!==root;
  frame.classList.toggle('viewing-subagent',child||!!data.parent);
  context.hidden=child||!!data.parent||!context.querySelector('.sidebar-section');
  frame.classList.toggle('without-context',context.hidden);
  breadcrumb.hidden=!child&&!data.parent;
  back.href=`/agent/${data.parent||root}`;
  const ancestors:string[]=[];let ancestor=data;
  const visited=new Set<string>();
  while(ancestor.id!==root&&!visited.has(ancestor.id)){visited.add(ancestor.id);ancestors.unshift(ancestor.title);ancestor=views.find(v=>v.id===ancestor.parent)!;if(!ancestor)break;}
  breadcrumb.querySelector('.breadcrumb-trail')!.textContent=ancestors.join(' / ');
  frame.querySelector('.session-id')!.textContent=id;
  frame.querySelector<HTMLElement>('.branch-name')!.hidden=!data.branch;
  frame.querySelector('[data-branch]')!.textContent=data.branch||'';
  const commits=frame.querySelector('[data-commits]')!;commits.replaceChildren();
  for(const commit of data.commits){const a=document.createElement('a');a.className='commit-ref';a.href=`#${child?id+'-':''}message-${commit.evidenceMessageId}`;a.textContent=commit.sha.slice(0,8);a.ariaLabel=`Commit ${commit.sha}; view transcript evidence`;commits.append(a);}
  frame.querySelector('[data-session-count]')!.textContent=`${data.turns} human turn${data.turns===1?'':'s'} · ${data.messages} messages`;
  const date=frame.querySelector<HTMLTimeElement>('.session-start time')!;date.dateTime=data.startedAt;date.title=fullTimestamp(data.startedAt);date.textContent=absoluteDate(data.startedAt);
  const relative=frame.querySelector<HTMLElement>('[data-relative-date]')!;relative.textContent=relativeAge(data.startedAt);relative.title=fullTimestamp(data.startedAt);
  document.title=`${data.title} — Token sessions`;outlineLabel();
  if(id!==root&&(loadedChunks.get(id)??0)===0)void appendChunk(id);
  pane.scrollTo({top:positions.get(id)||0,behavior:'instant'});
  if(focus){const target=openers.get(id)||pane.querySelector<HTMLElement>('h1')||pane;target.setAttribute('tabindex','-1');target.focus({preventScroll:true});}
  return true;
}
function navigate(id:string){
  const previous=active;
  if(!show(id,true))return false;
  history.pushState({sessionRoot:root,sessionId:id,previous},'',`/agent/${id}${location.search}`);return true;
}
function canonicalHash(target:string){
  const source=sourceMessageId(target,active);
  return source?`message-${source}`:target;
}
async function jump(id:string,smooth=true,preferred=root){
  const generation=navigationGeneration;
  let target=document.getElementById(id);
  if(!target){
    const location=anchorLocation(id,preferred);if(!location)return false;
    if(!await ensureChunk(location.id,location.page))return false;
    if(generation!==navigationGeneration)return false;
    target=document.getElementById(location.target);if(!target)return false;
  }
  const pane=target.closest<HTMLElement>('[data-view]');if(!pane)return false;
  if(pane.dataset.view!==active){navigate(pane.dataset.view!);}
  pane.querySelectorAll('.message-selected').forEach(message=>message.classList.remove('message-selected'));
  const message=target.closest('.message-group');if(message)message.classList.add('message-selected');
  const tool=target.querySelector<HTMLDetailsElement>('.tool-call');if(tool)tool.open=true;
  if(mobile.matches)nav.open=false;
  const bounds=(message||target).getBoundingClientRect();
  const inset=32;
  const offset=Math.max(inset,(pane.clientHeight-bounds.height)/2);
  pane.scrollTo({top:pane.scrollTop+bounds.top-pane.getBoundingClientRect().top-offset,behavior:smooth&&!matchMedia('(prefers-reduced-motion: reduce)').matches?'smooth':'instant'});
  target.setAttribute('tabindex','-1');target.focus({preventScroll:true});return true;
}
frame.closest('main')!.addEventListener('click',event=>{
  const a=(event.target as Element).closest<HTMLAnchorElement>('a');
  if(!a||event.ctrlKey||event.metaKey||event.shiftKey||event.altKey)return;
  if(a.matches('[data-load-more]')){
    const id=a.dataset.sessionId!,next=Number.parseInt(a.dataset.nextChunk||'',10);
    if(id&&Number.isInteger(next)){event.preventDefault();void appendChunk(id,next);}
    return;
  }
  if(a.matches('[data-subagent]')){const id=a.dataset.subagent!;if(!views.some(v=>v.id===id))return;openers.set(active,a);event.preventDefault();navigate(id);return;}
  if(a.matches('[data-parent-back]')){const id=views.find(v=>v.id===active)?.parent;if(id&&views.some(v=>v.id===id)){event.preventDefault();if(history.state?.previous===id)history.back();else navigate(id);}return;}
  if(a.hash==='#transcript'){event.preventDefault();panes.get(active)!.focus({preventScroll:true});return;}
  if(a.getAttribute('href')?.startsWith('#')){
    event.preventDefault();
    const preferred=a.closest('.transcript-navigation,.session-context')?root:active;
    const source=sourceMessageId(a.hash.slice(1),a.closest<HTMLElement>('[data-view]')?.dataset.view||active);
    void jump(a.hash.slice(1),true,preferred).then(jumped=>{
      if(jumped)history.replaceState(history.state,'',location.pathname+location.search+'#'+(source?`message-${source}`:canonicalHash(a.hash.slice(1))));
    });
  }
});
history.replaceState({sessionRoot:root,sessionId:root},'',location.href);
show(root);
function resolveHash(smooth=true){if(location.hash)void jump(location.hash.slice(1),smooth,active);}
window.addEventListener('popstate',()=>{
  const id=decodeURIComponent(location.pathname.split('/').filter(Boolean).at(-1)!);
  if(!show(id,true)){location.reload();return;}
  // Cached parent panes retain their exact reading position. Only resolve a
  // history hash when its target has not been rendered yet (for example a
  // lazily loaded child or a deferred transcript chunk).
  if(location.hash&&!document.getElementById(location.hash.slice(1)))resolveHash(false);
});
window.addEventListener('hashchange',()=>resolveHash());
requestAnimationFrame(()=>resolveHash(false));
