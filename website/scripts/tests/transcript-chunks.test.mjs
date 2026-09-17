import assert from 'node:assert/strict';
import test from 'node:test';
import {TRANSCRIPT_CHUNK_SIZE,transcriptChunk,transcriptChunkPath,transcriptChunks} from '../../src/lib/transcript-chunks.mjs';

const session={id:'session-with-gaps',messages:Array.from({length:205},(_,index)=>({
  // Importers preserve source ids. Deliberately skip numeric ordinals here.
  id:`m${String((index+1)*3).padStart(4,'0')}`,
  ordinal:(index+1)*3,
}))};

test('chunk metadata maps source ids with ordinal gaps to their static page',()=>{
  const meta=transcriptChunks(session);
  assert.equal(meta.size,TRANSCRIPT_CHUNK_SIZE);
  assert.equal(meta.count,3);
  assert.equal(meta.messagePages.m0003,0);
  assert.equal(meta.messagePages.m0300,0);
  assert.equal(meta.messagePages.m0303,1);
  assert.equal(meta.messagePages.m0615,2);
});

test('chunks retain source order and keep only the requested bounded page',()=>{
  assert.deepEqual(transcriptChunk(session,0).messages.map(message=>message.id).slice(0,2),['m0003','m0006']);
  assert.equal(transcriptChunk(session,1).messages.length,100);
  assert.equal(transcriptChunk(session,2).messages.length,5);
  assert.equal(transcriptChunk(session,2).messages[0].id,'m0603');
});

test('static fragment paths safely encode individual session ids',()=>{
  assert.equal(transcriptChunkPath('codex:child',2),'/agent-chunks/codex:child/2');
  assert.equal(transcriptChunkPath('codex/child',2),'/agent-chunks/codex%2Fchild/2');
});
