import MarkdownIt from "markdown-it";
import hljs from "highlight.js";

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function highlightCode(str: string, lang: string): string {
  if (lang && hljs.getLanguage(lang)) {
    try {
      return `<pre class="hljs"><code>${hljs.highlight(str, { language: lang, ignoreIllegals: true }).value}</code></pre>`;
    } catch {
      /* fall through */
    }
  }
  return `<pre class="hljs"><code>${escapeHtml(str)}</code></pre>`;
}

const md = new MarkdownIt({
  html: false,
  breaks: true,
  linkify: true,
  typographer: true,
  highlight: highlightCode,
});

md.validateLink = function (url: string): boolean {
  return /^(https?:\/\/|mailto:)/i.test(url);
};

// Open external links via Tauri shell plugin or window.open fallback
md.renderer.rules.link_open = function (tokens, idx, options, env, self) {
  const token = tokens[idx];
  const href = token.attrGet("href");
  if (href && /^https?:\/\//i.test(href)) {
    token.attrSet("target", "_blank");
    token.attrSet("rel", "noopener noreferrer");
  }
  return self.renderToken(tokens, idx, options);
};

const THINK_RE = /<think>([\s\S]*?)<\/think>/g;

export function renderMarkdown(text: string): string {
  const blocks: string[] = [];
  const cleaned = text.replace(THINK_RE, (_, content) => {
    blocks.push(content.trim());
    return `\x00THINK_${blocks.length - 1}\x00`;
  });

  let html = md.render(cleaned);

  html = html.replace(/\x00THINK_(\d+)\x00/g, (_, idx) => {
    const rendered = md.render(blocks[+idx]);
    return `<details class="think-block"><summary class="think-summary">🤔 思考过程</summary><div class="think-content">${rendered}</div></details>`;
  });

  return html;
}

export function handleLinkClick(e: MouseEvent): void {
  const target = e.target as HTMLElement;
  const anchor = target.closest("a");
  if (!anchor) return;
  const href = anchor.getAttribute("href");
  if (!href || !/^https?:\/\//i.test(href)) return;
  e.preventDefault();
  import("@tauri-apps/plugin-shell")
    .then((mod) => mod.open(href))
    .catch(() => {
      window.open(href, "_blank", "noopener,noreferrer");
    });
}
