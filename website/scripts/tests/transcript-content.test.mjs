import assert from 'node:assert/strict';
import test from 'node:test';
import {claudeTaskNotification,claudeTaskOutputReference,inlineTokens,markdownBlocks,messageSegments,publicationOmissions,safeExternalHref,toolPresentation} from '../../src/lib/transcript-content.mjs';

test('renders structured Markdown without manufacturing HTML',()=>{
  assert.deepEqual(markdownBlocks('# Heading\n\n- one\n- two\n\n```rs\nfn main() {}\n```'),[
    {type:'heading',level:1,value:'Heading'},
    {type:'list',items:['one','two']},
    {type:'code',language:'rs',value:'fn main() {}'},
  ]);
  assert.deepEqual(inlineTokens('Run `just test` and [docs](https://example.test/docs).'),[
    {type:'text',value:'Run '},
    {type:'code',value:'just test'},
    {type:'text',value:' and '},
    {type:'link',value:'docs',href:'https://example.test/docs'},
    {type:'text',value:'.'},
  ]);
});

test('refuses unsafe Markdown links while preserving their readable label',()=>{
  assert.equal(safeExternalHref('javascript:alert(1)'),null);
  assert.deepEqual(inlineTokens('[unsafe](javascript:bad)'),[{type:'text',value:'unsafe'}]);
  assert.equal(safeExternalHref('https://example.test/a'), 'https://example.test/a');
});

test('renders a basic GFM pipe table with alignment and escaped cell content',()=>{
  assert.deepEqual(markdownBlocks('| Icon | Mapping |\n| :--- | ---: |\n| `\\|` | Nerd Font \\| fallback |'),[
    {type:'table',headers:['Icon','Mapping'],alignments:['left','right'],rows:[['`|`','Nerd Font | fallback']]},
  ]);
});

test('marks embedded provider reasoning explicitly instead of dropping it',()=>{
  assert.deepEqual(messageSegments('[Thinking]\nRead the files.\n[/Thinking]\nI found the issue.'),[
    {type:'thinking',value:'Read the files.'},
    {type:'markdown',value:'I found the issue.'},
  ]);
  assert.deepEqual(messageSegments('Ordinary response'),[{type:'markdown',value:'Ordinary response'}]);
});

test('reports only explicit publication omissions',()=>{
  assert.deepEqual(publicationOmissions({publication:{omitted:{unmatchedResultEvents:2,attachmentReferences:1,dedicatedThinkingMessages:0}}}),[
    '2 unmatched tool result events','1 attachment reference',
  ]);
  assert.deepEqual(publicationOmissions({}),[]);
});

test('presents recorded tool states without inferring an outcome from output prose',()=>{
  assert.deepEqual(toolPresentation('completed'),{state:'completed',label:'Completed',symbol:'✓'});
  assert.deepEqual(toolPresentation('failed'),{state:'failed',label:'Failed',symbol:'×'});
  assert.deepEqual(toolPresentation('pending'),{state:'pending',label:'Pending',symbol:'…'});
  assert.deepEqual(toolPresentation('cancelled'),{state:'failed',label:'Cancelled',symbol:'×'});
  assert.deepEqual(toolPresentation('timed_out'),{state:'failed',label:'Timed out',symbol:'×'});
  assert.deepEqual(toolPresentation('running'),{state:'pending',label:'Running',symbol:'…'});
  assert.deepEqual(toolPresentation('queued'),{state:'pending',label:'Queued',symbol:'…'});
  assert.deepEqual(toolPresentation(''),{state:'unknown',label:'Status unavailable',symbol:'·'});
  assert.deepEqual(toolPresentation('the command mentioned an error'),{state:'unknown',label:'Status unavailable',symbol:'·'});
});

test('recognizes only Claude’s full temporary task-output Read notation',()=>{
  const reference=claudeTaskOutputReference('[Read: /private/tmp/claude-501/-Users-helge-code-token-editor/02e98a86-449d-4e86-bc96-550535b2a5dc/tasks/boyuauk9i.output]');
  assert.deepEqual(reference,{
    path:'/private/tmp/claude-501/-Users-helge-code-token-editor/02e98a86-449d-4e86-bc96-550535b2a5dc/tasks/boyuauk9i.output',
    sessionId:'02e98a86-449d-4e86-bc96-550535b2a5dc',
    taskId:'boyuauk9i',
  });
  assert.equal(claudeTaskOutputReference('Read: /private/tmp/claude-501/task.output'),null);
  assert.equal(claudeTaskOutputReference('[Read: /private/tmp/claude-501/-Users-helge-code-token-editor/02e98a86-449d-4e86-bc96-550535b2a5dc/scratchpad/shots/example.png]'),null);
});

test('extracts Claude background-task state only from a complete task notification',()=>{
  const notification=claudeTaskNotification(`<task-notification>
<task-id>boyuauk9i</task-id>
<tool-use-id>toolu_014WsXWvdho4KuxjvQsfRZjG</tool-use-id>
<output-file>/private/tmp/claude-501/-Users-helge-code-token-editor/02e98a86-449d-4e86-bc96-550535b2a5dc/tasks/boyuauk9i.output</output-file>
<status>completed</status>
<summary>Background command completed (exit code 0)</summary>
</task-notification>`);
  assert.deepEqual(notification,{
    taskId:'boyuauk9i',
    outputPath:'/private/tmp/claude-501/-Users-helge-code-token-editor/02e98a86-449d-4e86-bc96-550535b2a5dc/tasks/boyuauk9i.output',
    status:'completed',
    summary:'Background command completed (exit code 0)',
    toolUseId:'toolu_014WsXWvdho4KuxjvQsfRZjG',
    source:`<task-notification>
<task-id>boyuauk9i</task-id>
<tool-use-id>toolu_014WsXWvdho4KuxjvQsfRZjG</tool-use-id>
<output-file>/private/tmp/claude-501/-Users-helge-code-token-editor/02e98a86-449d-4e86-bc96-550535b2a5dc/tasks/boyuauk9i.output</output-file>
<status>completed</status>
<summary>Background command completed (exit code 0)</summary>
</task-notification>`,
  });
  assert.equal(claudeTaskNotification('<task-notification><task-id>missing-output</task-id></task-notification>'),null);
});
