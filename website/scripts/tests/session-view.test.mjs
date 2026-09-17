import test from 'node:test';
import assert from 'node:assert/strict';
import {detailTitle,rawTitle} from '../../src/lib/session-view.mjs';

test('detail titles recover full derived prompts without expanding named titles',()=>{
  const text='A long prompt '.repeat(30)+'\nIts final requirement.';
  const session={id:'s',titleSource:'first_human_turn',title:'A long prompt…',messages:[{role:'system',text:'Instructions'},{role:'user',text}]};
  assert.equal(detailTitle(session),text);
  assert.equal(rawTitle(session),'A long prompt…');
  assert.equal(detailTitle({...session,titleSource:'session_name',title:'Recorded name'}),'Recorded name');
  assert.equal(detailTitle({...session,title:null}),text);
  assert.equal(detailTitle({id:'empty',messages:[]}),'empty');
});
