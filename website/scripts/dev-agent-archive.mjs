import {spawn} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {resolve} from 'node:path';
import {existsSync} from 'node:fs';
import {homedir} from 'node:os';

const website = fileURLToPath(new URL('../', import.meta.url));
const archive = resolve(process.env.AGENT_ARCHIVE_PREVIEW || resolve(homedir(), '.local/share/token-agent-archive/preview'));
if (!existsSync(resolve(archive, 'index.json'))) {
  console.error('No real transcript preview found. Run normalize_archive.py export-preview first.');
  process.exit(1);
}
const child = spawn(process.execPath, [resolve(website, 'node_modules/astro/bin/astro.mjs'), 'dev', '--host', '127.0.0.1', ...process.argv.slice(2)], {
  cwd: website, stdio: 'inherit', env: {...process.env, AGENT_ARCHIVE_PREVIEW: archive},
});
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
child.on('exit', code => process.exit(code ?? 1));
