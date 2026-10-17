// Syntax highlighting for diffs, loaded on demand (see `useHighlighter`) so it never
// delays a window's first paint. Only the languages below are bundled.

import hljs from 'highlight.js/lib/core';
import bash from 'highlight.js/lib/languages/bash';
import c from 'highlight.js/lib/languages/c';
import cpp from 'highlight.js/lib/languages/cpp';
import csharp from 'highlight.js/lib/languages/csharp';
import css from 'highlight.js/lib/languages/css';
import dart from 'highlight.js/lib/languages/dart';
import dockerfile from 'highlight.js/lib/languages/dockerfile';
import elixir from 'highlight.js/lib/languages/elixir';
import go from 'highlight.js/lib/languages/go';
import graphql from 'highlight.js/lib/languages/graphql';
import ini from 'highlight.js/lib/languages/ini';
import java from 'highlight.js/lib/languages/java';
import javascript from 'highlight.js/lib/languages/javascript';
import json from 'highlight.js/lib/languages/json';
import kotlin from 'highlight.js/lib/languages/kotlin';
import lua from 'highlight.js/lib/languages/lua';
import makefile from 'highlight.js/lib/languages/makefile';
import markdown from 'highlight.js/lib/languages/markdown';
import php from 'highlight.js/lib/languages/php';
import python from 'highlight.js/lib/languages/python';
import ruby from 'highlight.js/lib/languages/ruby';
import rust from 'highlight.js/lib/languages/rust';
import scala from 'highlight.js/lib/languages/scala';
import scss from 'highlight.js/lib/languages/scss';
import sql from 'highlight.js/lib/languages/sql';
import swift from 'highlight.js/lib/languages/swift';
import typescript from 'highlight.js/lib/languages/typescript';
import xml from 'highlight.js/lib/languages/xml';
import yaml from 'highlight.js/lib/languages/yaml';

const LANGUAGES = {
    bash,
    c,
    cpp,
    csharp,
    css,
    dart,
    dockerfile,
    elixir,
    go,
    graphql,
    ini,
    java,
    javascript,
    json,
    kotlin,
    lua,
    makefile,
    markdown,
    php,
    python,
    ruby,
    rust,
    scala,
    scss,
    sql,
    swift,
    typescript,
    xml,
    yaml,
};

for (const [name, language] of Object.entries(LANGUAGES)) {
    hljs.registerLanguage(name, language);
}

type Language = keyof typeof LANGUAGES;

const BY_EXTENSION: Record<string, Language> = {
    bash: 'bash',
    c: 'c',
    cc: 'cpp',
    cjs: 'javascript',
    cpp: 'cpp',
    cs: 'csharp',
    css: 'css',
    cts: 'typescript',
    cxx: 'cpp',
    dart: 'dart',
    ex: 'elixir',
    exs: 'elixir',
    go: 'go',
    gql: 'graphql',
    gradle: 'kotlin',
    graphql: 'graphql',
    h: 'c',
    hpp: 'cpp',
    htm: 'xml',
    html: 'xml',
    ini: 'ini',
    java: 'java',
    js: 'javascript',
    json: 'json',
    jsonc: 'json',
    jsx: 'javascript',
    kt: 'kotlin',
    kts: 'kotlin',
    lua: 'lua',
    markdown: 'markdown',
    md: 'markdown',
    mdx: 'markdown',
    mjs: 'javascript',
    mts: 'typescript',
    php: 'php',
    plist: 'xml',
    py: 'python',
    pyi: 'python',
    rb: 'ruby',
    rs: 'rust',
    scala: 'scala',
    scss: 'scss',
    sh: 'bash',
    sql: 'sql',
    svg: 'xml',
    swift: 'swift',
    toml: 'ini',
    ts: 'typescript',
    tsx: 'typescript',
    vue: 'xml',
    xml: 'xml',
    yaml: 'yaml',
    yml: 'yaml',
    zsh: 'bash',
};

const BY_NAME: Record<string, Language> = {
    '.bashrc': 'bash',
    '.zshrc': 'bash',
    Dockerfile: 'dockerfile',
    Gemfile: 'ruby',
    GNUmakefile: 'makefile',
    Makefile: 'makefile',
    Rakefile: 'ruby',
};

/**
 * Highlights `lines` as one block, so strings and comments that span lines keep their
 * colour, and returns one HTML string per line. highlight.js escapes the text; a span
 * still open at a line break is closed there and reopened on the next line.
 */
export function highlightLines(lines: string[], language: Language): string[] {
    const html = hljs.highlight(lines.join('\n'), { ignoreIllegals: true, language }).value;
    const result: string[] = [];
    const open: string[] = [];
    let current = '';
    let index = 0;

    while (index < html.length) {
        if (html.startsWith('<span', index)) {
            const end = html.indexOf('>', index) + 1;
            const tag = html.slice(index, end);

            open.push(tag);
            current += tag;
            index = end;
        } else if (html.startsWith('</span>', index)) {
            open.pop();
            current += '</span>';
            index += 7;
        } else if (html[index] === '\n') {
            result.push(current + '</span>'.repeat(open.length));
            current = open.join('');
            index += 1;
        } else {
            current += html[index];
            index += 1;
        }
    }

    result.push(current + '</span>'.repeat(open.length));

    return result;
}

export function languageFor(path: string): Language | null {
    const name = path.split('/').pop() ?? '';
    const extension = name.includes('.') ? name.split('.').pop()!.toLowerCase() : '';

    return BY_NAME[name] ?? BY_EXTENSION[extension] ?? null;
}
