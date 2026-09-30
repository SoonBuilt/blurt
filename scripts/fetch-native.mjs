// Downloads the prebuilt sherpa-onnx static libraries (which include ONNX Runtime 1.28)
// into src-tauri/vendor/native/lib. Both sherpa-onnx and `ort` link against this one copy,
// so Parakeet, Pocket TTS, Silero VAD, Smart Turn and emotion2vec share a single runtime.
//
//   node scripts/fetch-native.mjs                       # for this machine
//   node scripts/fetch-native.mjs x86_64-apple-darwin   # when cross-compiling
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync, renameSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const SHERPA_VERSION = "1.13.8";
const root = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "vendor", "native");

const host = `${process.arch === "arm64" ? "aarch64" : "x86_64"}-${
  process.platform === "darwin" ? "apple-darwin" : process.platform === "win32" ? "pc-windows-msvc" : "unknown-linux-gnu"
}`;
const target = process.argv[2] || process.env.BLURT_TARGET || host;

const archives = {
  "aarch64-apple-darwin": "osx-arm64-static-lib",
  "x86_64-apple-darwin": "osx-x64-static-lib",
  "x86_64-pc-windows-msvc": "win-x64-static-MT-Release-lib",
  "aarch64-pc-windows-msvc": "win-arm64-static-MT-Release-lib",
  "x86_64-unknown-linux-gnu": "linux-x64-static-lib",
};
const flavour = archives[target];
if (!flavour) throw new Error(`No sherpa-onnx build for ${target}`);

const stamp = join(root, "TARGET");
const wanted = `${SHERPA_VERSION} ${target}`;
if (existsSync(stamp) && readFileSync(stamp, "utf8").trim() === wanted && existsSync(join(root, "lib"))) {
  console.log(`native libs already present for ${target}`);
  process.exit(0);
}

rmSync(root, { recursive: true, force: true });
mkdirSync(root, { recursive: true });
const name = `sherpa-onnx-v${SHERPA_VERSION}-${flavour}`;
const url = `https://github.com/k2-fsa/sherpa-onnx/releases/download/v${SHERPA_VERSION}/${name}.tar.bz2`;
const file = join(root, `${name}.tar.bz2`);
console.log(`downloading ${url}`);
const res = await fetch(url);
if (!res.ok) throw new Error(`download failed: ${res.status}`);
writeFileSync(file, Buffer.from(await res.arrayBuffer()));
execFileSync("tar", ["-xjf", file, "-C", root], { stdio: "inherit" });
rmSync(file);
// The archive unpacks to <name>/lib; flatten to vendor/native/lib.
const unpacked = readdirSync(root).find((d) => d.startsWith("sherpa-onnx-v"));
renameSync(join(root, unpacked, "lib"), join(root, "lib"));
rmSync(join(root, unpacked), { recursive: true, force: true });
writeFileSync(stamp, wanted + "\n");
console.log(`native libs ready for ${target} in ${join(root, "lib")}`);
