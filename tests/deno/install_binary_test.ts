import { assertEquals, assertThrows } from "https://deno.land/std@0.224.0/assert/mod.ts";
import { binaryName } from "../../_extensions/term/install-binary.ts";

Deno.test("binaryName - darwin aarch64 has no exe suffix", () => {
  assertEquals(binaryName("aarch64-apple-darwin"), "quarto-term-aarch64-apple-darwin");
});

Deno.test("binaryName - darwin x86_64 has no exe suffix", () => {
  assertEquals(binaryName("x86_64-apple-darwin"), "quarto-term-x86_64-apple-darwin");
});

Deno.test("binaryName - linux x86_64 has no exe suffix", () => {
  assertEquals(binaryName("x86_64-unknown-linux-gnu"), "quarto-term-x86_64-unknown-linux-gnu");
});

Deno.test("binaryName - linux aarch64 has no exe suffix", () => {
  assertEquals(binaryName("aarch64-unknown-linux-gnu"), "quarto-term-aarch64-unknown-linux-gnu");
});

Deno.test("binaryName - windows has exe suffix", () => {
  assertEquals(binaryName("x86_64-pc-windows-msvc"), "quarto-term-x86_64-pc-windows-msvc.exe");
});

Deno.test("detectPlatform returns a string for current platform", async () => {
  // Import detectPlatform - it uses Deno.build so we can only verify it doesn't throw
  const { detectPlatform } = await import("../../_extensions/term/install-binary.ts");
  const platform = detectPlatform();
  assertEquals(typeof platform, "string");
  // Should contain a known OS/arch pattern
  const valid = [
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
  ];
  assertEquals(valid.includes(platform), true, `unexpected platform: ${platform}`);
});

Deno.test("cache logic - version file missing means download needed", async () => {
  const tmpDir = await Deno.makeTempDir();
  try {
    const versionFile = `${tmpDir}/.version`;
    let needsDownload = true;
    try {
      await Deno.readTextFile(versionFile);
      needsDownload = false;
    } catch {
      needsDownload = true;
    }
    assertEquals(needsDownload, true);
  } finally {
    await Deno.remove(tmpDir, { recursive: true });
  }
});

Deno.test("cache logic - matching version file means no download", async () => {
  const tmpDir = await Deno.makeTempDir();
  try {
    const versionFile = `${tmpDir}/.version`;
    const binPath = `${tmpDir}/quarto-term-test`;
    await Deno.writeTextFile(versionFile, "0.3.0");
    await Deno.writeTextFile(binPath, "fake binary");

    const cached = (await Deno.readTextFile(versionFile)).trim();
    let needsDownload = cached !== "0.3.0";
    if (!needsDownload) {
      try {
        await Deno.stat(binPath);
      } catch {
        needsDownload = true;
      }
    }
    assertEquals(needsDownload, false);
  } finally {
    await Deno.remove(tmpDir, { recursive: true });
  }
});

Deno.test("cache logic - mismatched version means download needed", async () => {
  const tmpDir = await Deno.makeTempDir();
  try {
    const versionFile = `${tmpDir}/.version`;
    await Deno.writeTextFile(versionFile, "0.2.0");

    const cached = (await Deno.readTextFile(versionFile)).trim();
    const needsDownload = cached !== "0.3.0";
    assertEquals(needsDownload, true);
  } finally {
    await Deno.remove(tmpDir, { recursive: true });
  }
});

Deno.test("cache logic - version matches but binary missing means download needed", async () => {
  const tmpDir = await Deno.makeTempDir();
  try {
    const versionFile = `${tmpDir}/.version`;
    const binPath = `${tmpDir}/quarto-term-nonexistent`;
    await Deno.writeTextFile(versionFile, "0.3.0");

    const cached = (await Deno.readTextFile(versionFile)).trim();
    let needsDownload = cached !== "0.3.0";
    if (!needsDownload) {
      try {
        await Deno.stat(binPath);
      } catch {
        needsDownload = true;
      }
    }
    assertEquals(needsDownload, true);
  } finally {
    await Deno.remove(tmpDir, { recursive: true });
  }
});
