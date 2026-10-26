// Generates TypeScript declarations from the protocol's JSON Schemas.
//
//   node scripts/gen-types.mjs <schema-dir> <out-dir>
//
// Reads <schema-dir>/worker-bridge.schema.json (written by the Rust
// `verimap-schemas` binary) and writes <out-dir>/protocol.ts, replacing the
// directory's contents. `just gen` runs both steps; `just check-generated`
// runs them into a temporary directory and compares. The output types
// describe data; they do not validate it (the Rust core does).
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { compile } from 'json-schema-to-typescript';

const [schemaDir, outDir] = process.argv.slice(2);
if (!schemaDir || !outDir) {
  console.error('usage: node scripts/gen-types.mjs <schema-dir> <out-dir>');
  process.exit(2);
}

const schemaFile = 'worker-bridge.schema.json';
const schema = JSON.parse(await readFile(join(schemaDir, schemaFile), 'utf8'));
const version = schema['x-verimap-protocol-version'];
if (!Number.isInteger(version?.major) || !Number.isInteger(version?.minor)) {
  throw new Error(`${schemaFile}: missing x-verimap-protocol-version`);
}

// A `$ref` with sibling annotations (schemars puts field descriptions there)
// makes json-schema-to-typescript mint a numbered duplicate type per use
// site. Keep only the reference so each type is declared once; the field
// documentation remains in the schema and the Rust source.
function bareRefs(node) {
  if (Array.isArray(node)) return node.map(bareRefs);
  if (node === null || typeof node !== 'object') return node;
  if ('$ref' in node) return { $ref: node.$ref };
  return Object.fromEntries(Object.entries(node).map(([k, v]) => [k, bareRefs(v)]));
}

const types = await compile(bareRefs(schema), 'WorkerBridge', {
  bannerComment: [
    `// Generated from schemas/${schemaFile} by web/scripts/gen-types.mjs.`,
    '// Do not edit; run `just gen`. Types only: incoming data is validated in Rust.',
  ].join('\n'),
  additionalProperties: false,
  unreachableDefinitions: true,
  strictIndexSignatures: true,
  style: { singleQuote: true, printWidth: 100 },
});

const constant = `
/** Protocol version of the Rust core these types were generated from. */
export const PROTOCOL_VERSION: ProtocolVersion = { major: ${version.major}, minor: ${version.minor} };
`;

await rm(outDir, { recursive: true, force: true });
await mkdir(outDir, { recursive: true });
await writeFile(join(outDir, 'protocol.ts'), types + constant);
