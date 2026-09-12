import fs from 'node:fs';

const read = (path) => fs.readFileSync(path, 'utf8');
const pkg = JSON.parse(read('package.json'));
const pkgLock = JSON.parse(read('package-lock.json'));
const tauri = JSON.parse(read('src-tauri/tauri.conf.json'));
const cargoToml = read('src-tauri/Cargo.toml');
const cargoLock = read('src-tauri/Cargo.lock');

const cargoTomlVersion = cargoToml.match(/\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m)?.[1];
const cargoLockVersion = cargoLock.match(/\[\[package\]\]\s*\nname\s*=\s*"switchcraft"\s*\nversion\s*=\s*"([^"]+)"/m)?.[1];

const expected = (process.env.EXPECTED_VERSION || pkg.version).replace(/^v/, '');
const versions = {
  'package.json': pkg.version,
  'package-lock.json': pkgLock.version,
  'package-lock.json root package': pkgLock.packages?.['']?.version,
  'src-tauri/tauri.conf.json': tauri.version,
  'src-tauri/Cargo.toml': cargoTomlVersion,
  'src-tauri/Cargo.lock': cargoLockVersion,
};

const failures = Object.entries(versions).filter(([, value]) => value !== expected);

if (failures.length) {
  console.error(`SwitchCraft version mismatch. Expected ${expected}:`);
  for (const [source, value] of failures) {
    console.error(`- ${source}: ${value ?? 'missing'}`);
  }
  process.exit(1);
}

console.log(`SwitchCraft version metadata is synchronized at ${expected}.`);
