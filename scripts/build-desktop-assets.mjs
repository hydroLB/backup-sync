import { execFileSync } from 'node:child_process';
import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptRoot, '..');
const frontendRoot = join(repoRoot, 'crates', 'gui', 'frontend');
const binaryRoot = join(repoRoot, 'crates', 'gui', 'binaries');
const rustVersion = execFileSync('rustc', ['-vV'], { encoding: 'utf8' });
const hostTriple = rustVersion.match(/^host:\s+(\S+)$/m)?.[1];
const targetTriple = process.env.TAURI_ENV_TARGET_TRIPLE || hostTriple;

if (!targetTriple) {
  throw new Error('build-desktop-assets: could not resolve the Rust target triple');
}

const cargoArgs = [
  'build',
  '--manifest-path',
  join(repoRoot, 'Cargo.toml'),
  '-p',
  'daemon',
  '--release',
];
if (targetTriple !== hostTriple) cargoArgs.push('--target', targetTriple);
execFileSync('cargo', cargoArgs, { stdio: 'inherit' });

const windowsTarget = targetTriple.includes('windows');
const daemonName = windowsTarget ? 'daemon.exe' : 'daemon';
const daemonSource =
  targetTriple === hostTriple
    ? join(repoRoot, 'target', 'release', daemonName)
    : join(repoRoot, 'target', targetTriple, 'release', daemonName);
const bundledName = windowsTarget
  ? `daemon-${targetTriple}.exe`
  : `daemon-${targetTriple}`;
mkdirSync(binaryRoot, { recursive: true });
const bundledPath = join(binaryRoot, bundledName);
const sourceBytes = readFileSync(daemonSource);
const sidecarIsCurrent =
  existsSync(bundledPath) && sourceBytes.equals(readFileSync(bundledPath));
if (!sidecarIsCurrent) copyFileSync(daemonSource, bundledPath);
if (!windowsTarget) chmodSync(bundledPath, 0o755);

const frontendEnv = { ...process.env };
delete frontendEnv.VITE_APP_RUNTIME;
execFileSync(
  process.platform === 'win32' ? 'npm.cmd' : 'npm',
  ['--prefix', frontendRoot, 'run', 'build'],
  { stdio: 'inherit', env: frontendEnv },
);
