import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync, mkdirSync, writeFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {readPrivateArchive, privateArchiveRoot} from '../../src/lib/private-archive.mjs';

test('private preview follows its frozen receipt and ignores stale session files', () => {
  const root=mkdtempSync(join(tmpdir(),'token-archive-'));
  try {
    mkdirSync(join(root,'sessions'));
    const path='sessions/'+ 'a'.repeat(64)+'.json';
    writeFileSync(join(root,path),JSON.stringify({id:'codex:one',messages:[]}));
    writeFileSync(join(root,'sessions','b'.repeat(64)+'.json'),JSON.stringify({id:'stale'}));
    const receipt={purpose:'private-preview',sessions:[{id:'codex:one',path}]};
    writeFileSync(join(root,'index.json'),JSON.stringify(receipt));
    assert.equal(Object.keys(readPrivateArchive(root)).length,1);
    assert.equal(privateArchiveRoot({}),null);
    receipt.sessions[0].id='wrong';
    writeFileSync(join(root,'index.json'),JSON.stringify(receipt));
    assert.throws(()=>readPrivateArchive(root),/identity/);
    receipt.sessions[0].path='../outside.json';
    writeFileSync(join(root,'index.json'),JSON.stringify(receipt));
    assert.throws(()=>readPrivateArchive(root),/path/);
  } finally {rmSync(root,{recursive:true,force:true});}
});
