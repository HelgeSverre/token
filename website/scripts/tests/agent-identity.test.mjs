import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {AGENT_IDENTITIES,agentIconName,agentName} from '../../src/lib/agent-identity.mjs';

test('every registered harness has its local ACP SVG and display label',()=>{
 for(const [key,{icon,name}] of Object.entries(AGENT_IDENTITIES)) {
  assert.equal(agentName(key),name);
  assert.equal(agentIconName(key),icon);
  assert.match(readFileSync(new URL(`../../public/agent-icons/${icon}.svg`,import.meta.url),'utf8'),/<svg[ >]/);
 }
 assert.equal(agentIconName('antigravity-acp'),'antigravity');
 assert.equal(agentIconName('kimi-cli'),'kimi');
 assert.equal(agentIconName('unlisted harness'),null);
 assert.equal(agentName('unlisted harness'),'unlisted harness');
});
