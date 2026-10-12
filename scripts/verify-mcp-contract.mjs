// Inspect the actual sidecar without credentials or a business host.
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {isDeepStrictEqual} from 'node:util';

export async function readToolContract(executable) {
  const env = {...process.env};
  for (const key of Object.keys(env)) {
    if (/^IN_LINE_MCP_/i.test(key)) delete env[key];
  }
  const child = spawn(resolve(executable), [], {env, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe']});
  const pending = new Map();
  let failure;
  const fail = error => {
    failure = error;
    for (const item of pending.values()) item.reject(error);
    pending.clear();
  };
  child.on('error', fail);
  child.on('exit', () => fail(new Error('Sidecar exited before contract completed')));
  child.stderr.resume();
  const lines = createInterface({input: child.stdout});
  lines.on('line', line => {
    try {
      const message = JSON.parse(line);
      if (message.jsonrpc !== '2.0') throw new Error('Non-protocol stdout');
      const item = pending.get(message.id);
      if (item) { pending.delete(message.id); item.resolve(message); }
    } catch (error) { fail(error); }
  });
  const send = data => child.stdin.write(JSON.stringify(data) + '\n');
  const request = (id, method, params) => new Promise((resolve, reject) => {
    if (failure) return reject(failure);
    pending.set(id, {resolve, reject});
    send({jsonrpc: '2.0', id, method, params});
  });
  const timer = setTimeout(() => fail(new Error('Contract deadline exceeded')), 15000);
  try {
    const init = await request(1, 'initialize', {protocolVersion: '2025-11-25', capabilities: {}, clientInfo: {name: 'isolated-contract-regression', version: '1'}});
    if (!init.result) throw new Error('Initialization rejected');
    send({jsonrpc: '2.0', method: 'notifications/initialized'});
    const list = await request(2, 'tools/list', {});
    if (!Array.isArray(list.result?.tools)) throw new Error('Tool list missing');
    const result = list.result.tools.map(({name, inputSchema, annotations}) => ({name, inputSchema, annotations})).sort((a, b) => a.name.localeCompare(b.name));
    if (failure) throw failure;
    return result;
  } finally { clearTimeout(timer); lines.close(); child.kill(); }
}

export function assertToolContract(expected, actual) {
  if (!isDeepStrictEqual(expected, actual)) {
    const before = new Map(expected.map(tool => [tool.name, tool]));
    const after = new Map(actual.map(tool => [tool.name, tool]));
    const changed = [...new Set([...before.keys(), ...after.keys()])].filter(name => !isDeepStrictEqual(before.get(name), after.get(name)));
    throw new Error(`MCP contract changed: ${changed.join(', ')}. Review the breaking change and migration before updating the baseline; do not regenerate it to bypass this gate.`);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const baseline = JSON.parse(await readFile(new URL('../docs/mcp/contracts/tools-v1.json', import.meta.url), 'utf8'));
  const actual = await readToolContract(process.argv[2] ?? 'src-tauri/target/debug/in-line-mcp.exe');
  assertToolContract(baseline.tools, actual);
  console.log(`PASS: actual ${actual.length} tool names, complete input schemas and all protocol annotations match the reviewed API v1 baseline. No credentials/host/config writes.`);
}
