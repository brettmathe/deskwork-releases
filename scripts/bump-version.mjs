#!/usr/bin/env node
// Set the app version everywhere it appears, or verify the sites agree.
//
//   node scripts/bump-version.mjs 0.3.0     bump every site
//   node scripts/bump-version.mjs --check   verify they already agree
//   node scripts/bump-version.mjs 0.3.0 --dry-run
//
// tauri.conf.json is authoritative: getVersion() reports it, the updater
// compares against it, and the release workflow reads it to name the release.
// The rest are kept in step so nothing reports a stale number.

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const SEMVER = /^\d+\.\d+\.\d+$/;

const read = (rel) => readFileSync(join(ROOT, rel), "utf8");
const write = (rel, body) => writeFileSync(join(ROOT, rel), body);

/** Replace `version = "..."` inside Cargo.toml's [package] table only, never a
 *  dependency's inline version. */
function cargoTomlSetVersion(src, next) {
  const start = src.indexOf("[package]");
  if (start === -1) throw new Error("Cargo.toml has no [package] table");
  const after = src.indexOf("\n[", start + 1);
  const end = after === -1 ? src.length : after;
  const section = src.slice(start, end);
  const replaced = section.replace(/^version\s*=\s*"[^"]*"/m, `version = "${next}"`);
  if (replaced === section) throw new Error("no version key in [package]");
  return src.slice(0, start) + replaced + src.slice(end);
}

function cargoTomlGetVersion(src) {
  const start = src.indexOf("[package]");
  const after = src.indexOf("\n[", start + 1);
  const section = src.slice(start, after === -1 ? src.length : after);
  return section.match(/^version\s*=\s*"([^"]*)"/m)?.[1] ?? null;
}

/** The deskwork entry in Cargo.lock, which cargo rewrites for us. */
function cargoLockGetVersion(src) {
  return src.match(/\[\[package\]\]\nname = "deskwork"\nversion = "([^"]*)"/)?.[1] ?? null;
}

/** Every place a version is recorded, and how to read it. */
const SITES = [
  { file: "package.json", get: (s) => JSON.parse(s).version },
  { file: "package-lock.json", get: (s) => JSON.parse(s).version, label: "package-lock.json (root)" },
  {
    file: "package-lock.json",
    get: (s) => JSON.parse(s).packages?.[""]?.version,
    label: 'package-lock.json (packages[""])',
  },
  { file: "src-tauri/tauri.conf.json", get: (s) => JSON.parse(s).version, authoritative: true },
  { file: "src-tauri/Cargo.toml", get: cargoTomlGetVersion },
  { file: "src-tauri/Cargo.lock", get: cargoLockGetVersion },
];

function currentVersions() {
  return SITES.map((s) => ({ ...s, label: s.label ?? s.file, value: s.get(read(s.file)) }));
}

function report(versions) {
  const width = Math.max(...versions.map((v) => v.label.length));
  for (const v of versions) {
    const mark = v.authoritative ? " (authoritative)" : "";
    console.log(`  ${v.label.padEnd(width)}  ${v.value ?? "MISSING"}${mark}`);
  }
}

const args = process.argv.slice(2);
const check = args.includes("--check");
const dryRun = args.includes("--dry-run");
const next = args.find((a) => !a.startsWith("-"));

if (check) {
  const versions = currentVersions();
  report(versions);
  const distinct = new Set(versions.map((v) => v.value));
  if (distinct.size !== 1 || versions.some((v) => !v.value)) {
    console.error(`\nMISMATCH: ${[...distinct].join(", ")} — run: node scripts/bump-version.mjs <x.y.z>`);
    process.exit(1);
  }
  console.log(`\nAll sites agree on ${[...distinct][0]}.`);
  process.exit(0);
}

if (!next || !SEMVER.test(next)) {
  console.error("usage: node scripts/bump-version.mjs <x.y.z> [--dry-run]");
  console.error("       node scripts/bump-version.mjs --check");
  process.exit(2);
}

const before = currentVersions();
console.log("before:");
report(before);

if (dryRun) {
  console.log(`\n--dry-run: would set every site to ${next}`);
  process.exit(0);
}

// npm owns package.json + package-lock.json; it keeps both entries consistent.
execFileSync("npm", ["version", next, "--no-git-tag-version", "--allow-same-version"], {
  cwd: ROOT,
  stdio: "pipe",
});

const conf = JSON.parse(read("src-tauri/tauri.conf.json"));
conf.version = next;
write("src-tauri/tauri.conf.json", JSON.stringify(conf, null, 2) + "\n");

write("src-tauri/Cargo.toml", cargoTomlSetVersion(read("src-tauri/Cargo.toml"), next));

// Rewrites Cargo.lock's deskwork entry without compiling. `--no-deps` does NOT
// work here: it skips resolution, and resolution is what writes the lockfile.
execFileSync("cargo", ["metadata", "--format-version", "1"], {
  cwd: join(ROOT, "src-tauri"),
  // It prints ~1MB of JSON, which overflows the default maxBuffer. We only want
  // the lockfile side effect, so send stdout to the void and keep stderr.
  stdio: ["ignore", "ignore", "pipe"],
});

const after = currentVersions();
console.log("\nafter:");
report(after);

const wrong = after.filter((v) => v.value !== next);
if (wrong.length) {
  console.error(`\nFAILED: ${wrong.map((v) => v.label).join(", ")} did not reach ${next}`);
  process.exit(1);
}
console.log(`\nAll sites set to ${next}. Commit, then: git tag v${next} && git push origin v${next}`);
