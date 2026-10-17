import { useMemo } from 'react';

import { inlineHtml, markdownHtml } from '../../lib/visual';

/** One line of the model's markdown: `code`, **bold** and links, no blocks. */
export function Inline({ className = '', text }: { className?: string; text: string }) {
    const html = useMemo(() => inlineHtml(text), [text]);

    return <span className={`vz-prose ${className}`} dangerouslySetInnerHTML={{ __html: html }} />;
}

export function Prose({ markdown }: { markdown: string }) {
    const html = useMemo(() => markdownHtml(markdown), [markdown]);

    return <div className="vz-prose selectable" dangerouslySetInnerHTML={{ __html: html }} />;
}
