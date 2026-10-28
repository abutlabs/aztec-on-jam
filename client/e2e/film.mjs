// The README's two films. The intro, made by running this file: the opening crawl, its words
// from crawl.md, into intro.mp4 and intro.gif. The demo, made by explorer.mjs with RECORD=1:
// Alice's and Bob's wallets side by
// side, then the explorer, then the closing card; into demo.mp4 and demo.gif. Needs ffmpeg on PATH.
// Title scenes are drawn a frame at a time on the page's own clock, so they play the same in
// every run; the live parts are screencasts, sped up where the chain makes them wait.
//   node client/e2e/film.mjs        the intro (IMAGES=docs/images, CHROME=system Chrome); no net
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const W = 1100, H = 688, FPS = 8;             // the GIF's size and rate
const TITLES = pathToFileURL(path.join(HERE, "film.html")).href;
const CRAWL = path.join(HERE, "crawl.md");

/** crawl.md's words: its front matter's five lines, then paragraphs split at blank lines. */
export function crawlWords(file = CRAWL) {
  const m = readFileSync(file, "utf8").match(/^---\n([\s\S]*?)\n---\n([\s\S]*)$/);
  if (!m) throw new Error(`${file}: no front matter between --- lines`);
  const words = Object.fromEntries(m[1].split("\n").filter((l) => l.trim()).map((l) => {
    const i = l.indexOf(":");
    if (i < 0) throw new Error(`${file}: not "key: value": ${l}`);
    return [l.slice(0, i).trim(), l.slice(i + 1).trim()];
  }));
  for (const k of ["prelude", "title", "subtitle", "episode", "heading"]) if (!words[k]) throw new Error(`${file}: no ${k}`);
  words.paragraphs = m[2].split(/\n\s*\n/).map((p) => p.split("\n").map((l) => l.trim()).join(" ").trim()).filter(Boolean);
  if (!words.paragraphs.length) throw new Error(`${file}: no paragraphs`);
  return words;
}

export class Film {
  constructor(browser, dir) {
    this.browser = browser; this.dir = dir;
    this.parts = [];                           // in the order they play
    this.casts = {};                           // name -> { file, rec }
    rmSync(dir, { recursive: true, force: true }); mkdirSync(dir, { recursive: true });
  }

  /** One of film.html's scenes (crawl, card, end), drawn frame by frame. */
  async titles(scene) {
    const page = await this.browser.newPage();
    await page.setViewport({ width: W, height: H });
    // the card's net: lasair with the curve host calls on, or PolkaJam (FILM_NET, explorer.mjs)
    await page.goto(`${TITLES}?scene=${scene}&net=${process.env.FILM_NET || "polkajam"}`);
    if (scene === "crawl") await page.evaluate((w) => window.film.crawl(w), crawlWords());
    const { length, still } = await page.evaluate(() => ({ length: window.film.length, still: Boolean(window.film.still) }));
    const frames = path.join(this.dir, scene); mkdirSync(frames);
    const n = still ? 1 : Math.round(length * FPS);
    for (let i = 0; i < n; i++) {
      await page.evaluate((t) => window.film.at(t), i / FPS);
      await page.screenshot({ path: path.join(frames, `${String(i).padStart(5, "0")}.png`) });
    }
    await page.close();
    this.parts.push({ kind: still ? "still" : "frames", frames, length });
  }

  /** Start filming a live page. */
  async cast(name, page) {
    const file = path.join(this.dir, `${name}.webm`);
    this.casts[name] = { file, rec: await page.screencast({ path: file }) };
  }
  async stop(...names) { for (const n of names) await this.casts[n].rec.stop(); }
  /** Two casts side by side, `speed` times as fast. */
  side(left, right, speed) { this.parts.push({ kind: "side", files: [this.casts[left].file, this.casts[right].file], speed }); }
  /** One cast in the middle of the frame, `speed` times as fast. */
  centre(name, speed) { this.parts.push({ kind: "centre", files: [this.casts[name].file], speed }); }

  /** Everything, in order, into `mp4` and `gif`, both straight from the frames and casts: a GIF
   *  made from the MP4 carries its compression noise into every frame, three times the size. */
  render(mp4, gif) {
    const inputs = [], chains = [];
    const input = (...args) => { inputs.push(...args); return inputs.filter((a) => a === "-i").length - 1; };
    const out = (k) => `fps=${FPS},format=rgb24[p${k}]`;
    this.parts.forEach((p, k) => {
      if (p.kind === "frames") chains.push(`[${input("-framerate", String(FPS), "-i", path.join(p.frames, "%05d.png"))}:v]scale=${W}:${H},setsar=1,${out(k)}`);
      if (p.kind === "still") chains.push(`[${input("-loop", "1", "-framerate", String(FPS), "-t", String(p.length), "-i", path.join(p.frames, "00000.png"))}:v]scale=${W}:${H},setsar=1,${out(k)}`);
      if (p.kind === "side") {
        const [a, b] = p.files.map((f) => input("-i", f));
        const half = `scale=${W / 2}:${H}:force_original_aspect_ratio=decrease,setsar=1,pad=${W / 2}:${H}:(ow-iw)/2:0:color=0x05040b`;
        chains.push(`[${a}:v]setpts=PTS-STARTPTS,${half}[l${k}];[${b}:v]setpts=PTS-STARTPTS,${half}[r${k}];` +
          `[l${k}][r${k}]hstack=inputs=2,setpts=PTS/${p.speed},${out(k)}`);
      }
      if (p.kind === "centre") chains.push(`[${input("-i", p.files[0])}:v]setpts=PTS-STARTPTS,scale=-2:${H},setsar=1,` +
        `pad=${W}:${H}:(ow-iw)/2:0:color=0x05040b,setpts=PTS/${p.speed},${out(k)}`);
    });
    // one palette for the whole film (the crawl's yellow and the wallets' purples), and each GIF
    // frame only the rectangle that changed
    const graph = chains.join(";") + ";" + this.parts.map((_, k) => `[p${k}]`).join("") +
      `concat=n=${this.parts.length}:v=1:a=0,split[m][g];[g]format=rgb24,split[g0][g1];` +
      `[g0]palettegen=max_colors=64:stats_mode=diff[pal];[g1][pal]paletteuse=dither=none:diff_mode=rectangle[gif]`;
    execFileSync("ffmpeg", ["-y", "-loglevel", "error", ...inputs, "-filter_complex", graph,
      "-map", "[m]", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-movflags", "+faststart", mp4, "-map", "[gif]", gif]);
    rmSync(this.dir, { recursive: true, force: true });
  }
}

/** A caption above a live page's own header (so the page's name stays in view) that stays at the
 *  top as the page scrolls, in the rebels' colours; CAPTION px tall whatever the text, so the
 *  page below never moves. */
export const CAPTION = 92;
export async function caption(page, text) {
  await page.evaluate((t, h) => {
    let el = document.getElementById("film-caption");
    if (!el) {
      el = Object.assign(document.createElement("div"), { id: "film-caption" });
      el.style.cssText = `position:sticky;top:0;z-index:9999;display:flex;align-items:center;justify-content:center;height:${h}px;box-sizing:border-box;` +
        "padding:0 24px;background:#05040b;border-bottom:3px solid #ffe81f;color:#ffe81f;" +
        "font:700 26px/1.25 'Helvetica Neue',Helvetica,Arial,sans-serif;text-align:center";
      document.body.prepend(el);
    }
    el.textContent = t;
  }, text, CAPTION);
}
export const uncaption = (page) => page.evaluate(() => document.getElementById("film-caption")?.remove());

// the intro, when this file is run
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const { default: puppeteer } = await import("puppeteer-core");
  const IMAGES = process.env.IMAGES ? path.resolve(process.env.IMAGES) : path.resolve(HERE, "../../docs/images");
  const browser = await puppeteer.launch({ executablePath: process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", headless: true });
  const film = new Film(browser, path.join(IMAGES, "intro-parts"));
  await film.titles("crawl");
  film.render(path.join(IMAGES, "intro.mp4"), path.join(IMAGES, "intro.gif"));
  await browser.close();
  console.log(`intro ${path.relative(process.cwd(), IMAGES)}/intro.mp4 and intro.gif`);
}
