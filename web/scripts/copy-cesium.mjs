// Copies Cesium's static runtime assets (web workers, widget CSS, images,
// third-party WASM) into public/cesium/ so they are served and cached with the
// app. Nothing is ever loaded from a CDN. The copy is a build artefact
// (gitignored) and is refreshed whenever the installed Cesium changes.
import { cp, readFile, rm, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';

const require = createRequire(import.meta.url);
const cesiumRoot = dirname(require.resolve('cesium/package.json'));
const { version } = JSON.parse(await readFile(join(cesiumRoot, 'package.json'), 'utf8'));
const source = join(cesiumRoot, 'Build', 'Cesium');
const target = new URL('../public/cesium/', import.meta.url).pathname;
const stamp = join(target, '.version');

const current = await readFile(stamp, 'utf8').catch(() => null);
if (current !== version) {
  await rm(target, { recursive: true, force: true });
  for (const dir of ['Workers', 'ThirdParty', 'Assets', 'Widgets']) {
    await cp(join(source, dir), join(target, dir), { recursive: true });
  }
  await writeFile(stamp, version);
  console.log(`copied Cesium ${version} assets to public/cesium/`);
}
