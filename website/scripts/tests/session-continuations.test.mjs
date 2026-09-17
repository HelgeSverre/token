import test from 'node:test';
import assert from 'node:assert/strict';
import {ampContinuationReference,buildContinuationIndex,canonicalAmpThreadId,continuationContext} from '../../src/lib/session-continuations.mjs';

const first='T-11111111-1111-1111-1111-111111111111';
const second='T-22222222-2222-2222-2222-222222222222';
const third='T-33333333-3333-3333-3333-333333333333';
const fork='T-44444444-4444-4444-4444-444444444444';
const cycle='T-55555555-5555-5555-5555-555555555555';
const missing='T-99999999-9999-9999-9999-999999999999';

function amp(id, predecessor) {
  const prefix='<attached_files>\n</attached_files>\n\n';
  const handoff=predecessor ? `Continuing work from thread ${predecessor}. When you lack specific information you can use read_thread to get it.\n\nContinue.` : 'Start this task.';
  return {id,agent:'amp',displayTitle:`Session ${id.slice(2,10)}`,messages:[{id:`${id}:m1`,role:'user',text:prefix+handoff}]};
}

test('canonical Amp IDs unify historical T- and normalized amp:T- forms',()=>{
  assert.equal(canonicalAmpThreadId(`amp:${first.toUpperCase()}`),first);
  assert.equal(canonicalAmpThreadId(first),first);
  assert.equal(canonicalAmpThreadId('amp:not-a-thread'),null);
});

test('only Amp opening-message handoff boilerplate forms a continuation edge',()=>{
  const session=amp(`amp:${second}`,first);
  assert.deepEqual(ampContinuationReference(session),{threadId:first,evidenceMessageId:`amp:${second}:m1`});
  assert.equal(ampContinuationReference({...session,agent:'codex'}),null);
  assert.equal(ampContinuationReference({...session,messages:[{id:'m1',role:'user',text:`Please use read_thread to inspect ${first}.`}] }),null);
  assert.equal(ampContinuationReference({...session,messages:[{id:'m1',role:'user',text:`Continuing work from thread ${first}.`}] }),null);
  assert.equal(ampContinuationReference({...session,messages:[{id:'m1',role:'user',text:'Start.'},{id:'m2',role:'user',text:`Continuing work from thread ${first}. When you lack specific information you can use read_thread to get it.`}]}),null);
});

test('linear chains expose predecessor, successor, and a trustworthy ordinal',()=>{
  const index=buildContinuationIndex([amp(first),amp(`amp:${second}`,first),amp(third,second)]);
  const middle=continuationContext(index,`amp:${second}`);
  assert.equal(middle.predecessor.id,first);
  assert.deepEqual(middle.successors.map(s=>s.id),[third]);
  assert.deepEqual(middle.chain.map(s=>s.id),[first,`amp:${second}`,third]);
  assert.equal(middle.position,2);
  assert.equal(middle.total,3);
});

test('forks keep every explicit successor and avoid inventing a linear order',()=>{
  const index=buildContinuationIndex([amp(first),amp(second,first),amp(fork,first)]);
  const context=continuationContext(index,first);
  assert.deepEqual(context.successors.map(s=>s.id),[second,fork].sort());
  assert.equal(context.chain,null);
  assert.equal(context.position,null);
});

test('missing, ambiguous, and cyclic references never become navigation links',()=>{
  const unresolved=amp(second,missing);
  const duplicateA=amp(first);
  const duplicateB=amp(`amp:${first}`);
  const ambiguous=amp(third,first);
  const cycleA=amp(fork,cycle);
  const cycleB=amp(cycle,fork);
  const index=buildContinuationIndex([unresolved,duplicateA,duplicateB,ambiguous,cycleA,cycleB]);
  assert.equal(continuationContext(index,second).status,'unresolved');
  assert.equal(continuationContext(index,third).status,'ambiguous');
  assert.equal(continuationContext(index,fork).status,'cycle');
  assert.equal(continuationContext(index,cycle).status,'cycle');
  assert.equal(continuationContext(index,cycle).predecessor,null);
});
