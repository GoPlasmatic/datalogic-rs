/**
 * Copy the locally built WASM package (`../bindings/wasm/pkg`, produced by
 * `bindings/wasm/build.sh`) into `vendor/datalogic`, where every Vite,
 * Vitest and tsconfig alias resolves `@goplasmatic/datalogic-wasm`.
 *
 * Runs before dev, build, build:lib, build:embed and test. Plain Node fs
 * calls, so it works on Windows as well as POSIX shells.
 *
 * The engine's version must match this package's: the UI ships the engine
 * it was built against, and a stale copy would publish a mismatched one.
 * - With `pkg/` present, a version mismatch fails before anything is copied.
 * - Without `pkg/`, an existing `vendor/datalogic` of the right version is
 *   kept (with a warning); otherwise the script fails.
 * Set `DATALOGIC_ALLOW_WASM_VERSION_MISMATCH=1` to accept a mismatch.
 */
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const uiRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const source = join(uiRoot, '..', 'bindings', 'wasm', 'pkg');
const target = join(uiRoot, 'vendor', 'datalogic');
const BUILD_HINT = 'Build it with: cd bindings/wasm && ./build.sh';

/**
 * @param {string} dir
 * @returns {string | null}
 */
function readVersion(dir) {
  const manifest = join(dir, 'package.json');
  if (!existsSync(manifest)) return null;
  return JSON.parse(readFileSync(manifest, 'utf8')).version;
}

/** @param {string} message */
function fail(message) {
  console.error(`sync-wasm: ${message}`);
  process.exit(1);
}

/**
 * @param {string} where
 * @param {string} found
 */
function checkVersion(where, found) {
  if (found === uiVersion) return;
  const message = `${where} is @goplasmatic/datalogic-wasm ${found}, but this package is ${uiVersion}.`;
  if (process.env.DATALOGIC_ALLOW_WASM_VERSION_MISMATCH === '1') {
    console.warn(`sync-wasm: ${message} Continuing (DATALOGIC_ALLOW_WASM_VERSION_MISMATCH=1).`);
    return;
  }
  fail(`${message} ${BUILD_HINT}, or set DATALOGIC_ALLOW_WASM_VERSION_MISMATCH=1.`);
}

const uiVersion = readVersion(uiRoot);
const sourceVersion = readVersion(source);

if (sourceVersion === null) {
  const vendoredVersion = readVersion(target);
  if (vendoredVersion === null) {
    fail(`bindings/wasm/pkg is missing and there is no vendor/datalogic copy. ${BUILD_HINT}.`);
  } else {
    checkVersion('vendor/datalogic', vendoredVersion);
    console.warn(
      `sync-wasm: bindings/wasm/pkg is missing; keeping vendor/datalogic (${vendoredVersion}).`,
    );
  }
} else {
  checkVersion('bindings/wasm/pkg', sourceVersion);
  rmSync(target, { recursive: true, force: true });
  mkdirSync(dirname(target), { recursive: true });
  cpSync(source, target, { recursive: true });
  console.log(`sync-wasm: copied @goplasmatic/datalogic-wasm ${sourceVersion} into vendor/datalogic`);
}
