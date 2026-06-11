// Downloads a static ffmpeg binary into src-tauri/binaries/ named by Rust target triple.
import { createWriteStream } from "node:fs";
import { mkdir, chmod, rm } from "node:fs/promises";
import { execSync } from "node:child_process";
import { pipeline } from "node:stream/promises";
import { Readable } from "node:stream";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outDir = join(root, "src-tauri", "binaries");

// Resolve the Rust host target triple (must match externalBin naming exactly).
const triple = execSync("rustc -Vv").toString().split("\n")
  .find((l) => l.startsWith("host:")).split(" ")[1].trim();

// Pinned static builds. Update these URLs if a source goes stale.
const SOURCES = {
  "x86_64-pc-windows-msvc": {
    url: "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip",
    inner: "ffmpeg.exe", ext: ".exe",
  },
  "aarch64-apple-darwin": {
    url: "https://www.osxexperts.net/ffmpeg711arm.zip",
    inner: "ffmpeg", ext: "",
  },
  "x86_64-apple-darwin": {
    url: "https://www.osxexperts.net/ffmpeg7intel.zip",
    inner: "ffmpeg", ext: "",
  },
};

const src = SOURCES[triple];
if (!src) throw new Error(`No ffmpeg source pinned for target ${triple}`);

await mkdir(outDir, { recursive: true });
const tmp = join(outDir, "_dl.zip");
console.log(`Downloading ffmpeg for ${triple} ...`);
const res = await fetch(src.url, { redirect: "follow" });
if (!res.ok) throw new Error(`Download failed: ${res.status} ${src.url}`);
await pipeline(Readable.fromWeb(res.body), createWriteStream(tmp));

// Extract just the ffmpeg binary. tar reads zips on Win10 17063+/macOS.
const dest = join(outDir, `ffmpeg-${triple}${src.ext}`);
const exDir = join(outDir, "_ex");
await rm(exDir, { recursive: true, force: true });
await mkdir(exDir, { recursive: true });
console.log(`Extracting ${src.inner} -> ${dest}`);
execSync(`tar -xf "${tmp}" -C "${exDir}"`, { stdio: "inherit" });

// Find the inner binary (it may be nested in a versioned folder).
const found = execSync(
  process.platform === "win32"
    ? `where /r "${exDir}" ${src.inner}`
    : `find "${exDir}" -name ${src.inner} -type f`
).toString().split("\n")[0].trim();
execSync(process.platform === "win32" ? `move /y "${found}" "${dest}"` : `mv "${found}" "${dest}"`);
if (process.platform !== "win32") await chmod(dest, 0o755);
await rm(tmp, { force: true });
await rm(exDir, { recursive: true, force: true });
console.log(`OK: ${dest}`);
