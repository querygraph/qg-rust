import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";

const root = path.resolve(import.meta.dirname);
const workspaceRoot = path.resolve(root, "../..");
const versionFile = path.join(root, "dist", "VERSION.md");
const metadata = path.join(root, "metadata.yaml");
const cover = path.join(root, "cover.md");
const manuscript = path.join(root, "manuscript.md");
const buildDir = path.join(root, "build");
const buildDiagramDir = path.join(buildDir, "diagrams");
const bookDiagramDir = path.join(root, "diagrams");
const blogDiagramDir = path.join(workspaceRoot, "docs/blog/assets/querygraph/diagrams");
const renderedCover = path.join(buildDir, "cover.rendered.md");
const rendered = path.join(buildDir, "manuscript.rendered.md");
const puppeteerConfig = path.join(root, "puppeteer-config.json");

await mkdir(buildDiagramDir, { recursive: true });
await mkdir(bookDiagramDir, { recursive: true });
await mkdir(blogDiagramDir, { recursive: true });
await rm(buildDiagramDir, { recursive: true, force: true });
await rm(bookDiagramDir, { recursive: true, force: true });
await rm(blogDiagramDir, { recursive: true, force: true });
await mkdir(buildDiagramDir, { recursive: true });
await mkdir(bookDiagramDir, { recursive: true });
await mkdir(blogDiagramDir, { recursive: true });

const readYamlString = (yaml, key) => {
  const match = yaml.match(new RegExp(`^${key}:\\s*"([^"]+)"\\s*$`, "m"));
  if (!match) {
    throw new Error(`Missing ${key} in ${metadata}`);
  }
  return match[1];
};

const readSimpleYamlValue = (yaml, key, sourcePath) => {
  const match = yaml.match(new RegExp(`^${key}:\\s*(.+?)\\s*$`, "m"));
  if (!match) {
    throw new Error(`Missing ${key} in ${sourcePath}`);
  }
  return match[1].replace(/^["']|["']$/g, "");
};

const versionSource = await readFile(versionFile, "utf8");
const kindleName = readSimpleYamlValue(versionSource, "kindle_name", versionFile);

const metadataSource = await readFile(metadata, "utf8");
const titleStem = readYamlString(metadataSource, "title_stem");
const coverValues = {
  title: readYamlString(metadataSource, "title"),
  titleStem,
  subtitle: readYamlString(metadataSource, "subtitle"),
  author: readYamlString(metadataSource, "author"),
  rights: readYamlString(metadataSource, "rights"),
  versionSubtitle: `covers ${kindleName}`,
};

const escapeHtml = (value) =>
  value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const escapeTypstMarkup = (value) =>
  value.replace(/\\/g, "\\\\").replace(/\[/g, "\\[").replace(/\]/g, "\\]");

const coverSource = await readFile(cover, "utf8");
const renderedCoverMarkdown = coverSource.replace(
  /\{\{(title|subtitle|author|rights|versionSubtitle)\}\}/g,
  (_match, key, offset) => {
    const before = coverSource.slice(0, offset);
    const typstFence = before.lastIndexOf("```{=typst}");
    const htmlFence = before.lastIndexOf("```{=html}");
    const markdownFence = before.lastIndexOf("```");
    const value = coverValues[key];
    if (typstFence > htmlFence && typstFence === markdownFence) {
      return escapeTypstMarkup(value);
    }
    if (htmlFence > typstFence && htmlFence === markdownFence) {
      return escapeHtml(value);
    }
    return value;
  },
);
await writeFile(renderedCover, renderedCoverMarkdown);

const copyFile = (source, target) => {
  const result = spawnSync("cp", [source, target], { stdio: "inherit" });
  if (result.status !== 0) {
    throw new Error(`cp failed from ${source} to ${target}`);
  }
};

const renderMermaid = (input, output, config) => {
  const args = ["-i", input, "-o", output, "-b", "transparent", "-p", puppeteerConfig, "-s", "2"];
  if (config) {
    args.push("-c", config);
  }
  const result = spawnSync("mmdc", args, { stdio: "inherit" });
  if (result.status !== 0) {
    throw new Error(`mmdc failed for ${input}`);
  }
};

const pngSize = (file) => {
  const buffer = readFileSync(file);
  if (buffer.toString("ascii", 1, 4) !== "PNG") {
    throw new Error(`Expected PNG output at ${file}`);
  }
  return {
    width: buffer.readUInt32BE(16),
    height: buffer.readUInt32BE(20),
  };
};

let referenceDiagramWidth = null;

const source = await readFile(manuscript, "utf8");
let diagramIndex = 0;
const renderedMarkdown = source.replace(
  /```mermaid\n([\s\S]*?)\n```/g,
  (_match, diagram) => {
    diagramIndex += 1;
    const stem = `diagram-${String(diagramIndex).padStart(2, "0")}`;
    const input = path.join(buildDiagramDir, `${stem}.mmd`);
    const output = path.join(buildDiagramDir, `${stem}.png`);
    const sourceText = `${diagram.trim()}\n`;
    writeFileSync(input, sourceText);
    renderMermaid(input, output);

    const { width } = pngSize(output);
    if (referenceDiagramWidth === null) {
      referenceDiagramWidth = width;
    }

    const visualScale = width / referenceDiagramWidth;
    if (visualScale > 1.05) {
      const fontSize = Math.round(16 * visualScale);
      const config = path.join(buildDiagramDir, `${stem}.json`);
      writeFileSync(
        config,
        `${JSON.stringify({
          theme: "default",
          themeVariables: {
            fontSize: `${fontSize}px`,
          },
          sequence: {
            fontSize,
            actorFontSize: fontSize,
            messageFontSize: fontSize,
            noteFontSize: fontSize,
          },
        })}\n`,
      );
      renderMermaid(input, output, config);
    }

    copyFile(input, path.join(bookDiagramDir, `${stem}.mmd`));
    copyFile(output, path.join(bookDiagramDir, `${stem}.png`));
    copyFile(input, path.join(blogDiagramDir, `${stem}.mmd`));
    copyFile(output, path.join(blogDiagramDir, `${stem}.png`));
    return `![Diagram ${diagramIndex}](diagrams/${stem}.png)`;
  },
);

await writeFile(rendered, renderedMarkdown);
console.log(`Rendered ${diagramIndex} Mermaid diagram(s) to ${rendered}`);
console.log(`Materialized diagrams in ${bookDiagramDir}`);
console.log(`Copied blog diagrams to ${blogDiagramDir}`);
console.log(`Rendered cover for ${kindleName} to ${renderedCover}`);
