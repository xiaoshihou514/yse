import MarkdownIt from "markdown-it";
import MarkdownItContainer from "markdown-it-container";
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

md.use(MarkdownItContainer, "details", {
  render(tokens: any[], idx: number): string {
    if (tokens[idx].nesting === 1) {
      const s = tokens[idx].info.trim().slice(8).trim();
      return `<details class="details-block"><summary class="details-summary">${s}</summary><div class="details-content">\n`;
    }
    return `</div></details>\n`;
  },
});

md.use(MarkdownItContainer, "think", {
  render(tokens: any[], idx: number): string {
    if (tokens[idx].nesting === 1) {
      return `<details class="details-block"><summary class="details-summary">🤔 思考过程</summary><div class="details-content">\n`;
    }
    return `</div></details>\n`;
  },
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

export function renderMarkdown(text: string): string {
  text = text.replace(/<think>([\s\S]*?)<\/think>/g, (_, c) => `:::think\n${c.trim()}\n:::`);
  return md.render(text);
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
