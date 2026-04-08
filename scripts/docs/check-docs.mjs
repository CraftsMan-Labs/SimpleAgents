import { promises as fs } from "node:fs";
import path from "node:path";

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..", "..");
const docsRoot = path.join(repoRoot, "docs");

const requiredSections = {
  "QUICKSTART.md": [/^##\s+Prerequisites\b/m, /^##\s+1\.\s+Add dependencies\b/m, /^##\s+Next Steps\b/m],
  "USAGE.md": [/^##\s+Prerequisites\b/m, /^##\s+Quick Path\b/m, /^##\s+Troubleshooting\b/m, /^##\s+Next Steps\b/m],
  "YAML_WORKFLOW_SYSTEM.md": [/^##\s+Prerequisites\b/m, /^##\s+Quick Path\b/m, /^##\s+Troubleshooting\b/m, /^##\s+Next Steps\b/m],
  "WORKFLOW_QUICKSTART.md": [/^##\s+Install\b/m, /^##\s+Create a YAML Workflow\b/m, /^##\s+Run It\b/m],
};

async function walkMarkdownFiles(dir) {
  const entries = await fs.readdir(dir, { withFileTypes: true });
  const out = [];
  for (const entry of entries) {
    if (entry.name === "node_modules" || entry.name === ".vitepress") {
      continue;
    }
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...(await walkMarkdownFiles(fullPath)));
      continue;
    }
    if (entry.isFile() && entry.name.endsWith(".md")) {
      out.push(fullPath);
    }
  }
  return out;
}

function stripLinkDecorations(raw) {
  const trimmed = raw.trim();
  const unwrapped = trimmed.startsWith("<") && trimmed.endsWith(">")
    ? trimmed.slice(1, -1)
    : trimmed;
  const withoutHash = unwrapped.split("#", 1)[0];
  const withoutQuery = withoutHash.split("?", 1)[0];
  return withoutQuery;
}

function skipLinkTarget(link) {
  return (
    link.length === 0 ||
    link.startsWith("#") ||
    link.startsWith("http://") ||
    link.startsWith("https://") ||
    link.startsWith("mailto:") ||
    link.startsWith("tel:") ||
    link.startsWith("data:") ||
    link.startsWith("@")
  );
}

async function pathExists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

function routeCandidates(routePath) {
  if (routePath === "/") {
    return [path.join(docsRoot, "index.md")];
  }

  const clean = routePath.replace(/^\//, "").replace(/\/$/, "");
  if (clean.endsWith(".md")) {
    return [path.join(docsRoot, clean)];
  }
  return [
    path.join(docsRoot, `${clean}.md`),
    path.join(docsRoot, clean, "index.md"),
  ];
}

function relativeCandidates(sourceFile, rawRelative) {
  const sourceDir = path.dirname(sourceFile);
  const clean = rawRelative.replace(/\/$/, "");
  const resolved = path.resolve(sourceDir, clean);

  if (path.extname(clean) !== "") {
    return [resolved];
  }
  return [
    `${resolved}.md`,
    path.join(resolved, "index.md"),
  ];
}

async function validateLinks(markdownFiles) {
  const errors = [];
  const markdownLinkPattern = /!?\[[^\]]*\]\(([^)]+)\)/g;

  for (const filePath of markdownFiles) {
    const contents = await fs.readFile(filePath, "utf8");
    const relFile = path.relative(repoRoot, filePath);
    for (const match of contents.matchAll(markdownLinkPattern)) {
      const targetRaw = match[1] ?? "";
      const target = stripLinkDecorations(targetRaw);
      if (skipLinkTarget(target)) {
        continue;
      }

      let candidates;
      if (target.startsWith("/")) {
        candidates = routeCandidates(target);
      } else {
        candidates = relativeCandidates(filePath, target);
      }

      let found = false;
      for (const candidate of candidates) {
        if (await pathExists(candidate)) {
          found = true;
          break;
        }
      }

      if (!found) {
        errors.push(`${relFile}: unresolved link target \`${targetRaw}\``);
      }
    }
  }
  return errors;
}

async function validateRequiredSections() {
  const errors = [];
  for (const [fileName, patterns] of Object.entries(requiredSections)) {
    const filePath = path.join(docsRoot, fileName);
    const contents = await fs.readFile(filePath, "utf8");
    for (const pattern of patterns) {
      if (!pattern.test(contents)) {
        errors.push(`docs/${fileName}: missing required section matching ${pattern}`);
      }
    }
  }
  return errors;
}

async function main() {
  const markdownFiles = await walkMarkdownFiles(docsRoot);
  const linkErrors = await validateLinks(markdownFiles);
  const sectionErrors = await validateRequiredSections();
  const errors = [...linkErrors, ...sectionErrors];

  if (errors.length > 0) {
    console.error("Docs quality checks failed:\n");
    for (const error of errors) {
      console.error(`- ${error}`);
    }
    process.exit(1);
  }

  console.log(`Docs quality checks passed for ${markdownFiles.length} markdown files.`);
}

main().catch((error) => {
  console.error("Failed to run docs quality checks:", error);
  process.exit(1);
});
