import type { Finding, Severity } from '../lib/types';

import { Icon } from './Icon';
import { Button } from './ui';

const severities: Record<Severity, { chip: string; label: string; rail: string }> = {
    check: { chip: 'bg-add-bg text-add-num', label: 'Check', rail: 'var(--add)' },
    note: { chip: 'bg-control text-muted', label: 'Note', rail: 'var(--dim)' },
    risk: { chip: 'bg-warn-bg text-warn', label: 'Risk', rail: 'var(--del)' },
};

/**
 * One review finding. Inline it sits under its diff line and stays in view while the
 * code scrolls sideways; in the findings list it is a button that jumps to the line.
 */
export function FindingCard({
    finding,
    inline,
    onDismiss,
    onFocus,
}: {
    finding: Finding;
    inline?: boolean;
    onDismiss(): void;
    onFocus?: () => void;
}) {
    const location = finding.line ? `${finding.file}:${finding.line}` : finding.file;

    return (
        <div className={inline ? 'sticky left-0 py-1.5 pr-3 pl-[108px] font-sans' : 'font-sans'}>
            <article
                className="flex max-w-[720px] flex-col gap-2 rounded-lg border border-accent-line bg-accent-bg px-3 py-2.5 text-[13px]"
                style={{ boxShadow: `inset 3px 0 0 ${severities[finding.severity].rail}` }}
            >
                <header className="flex items-center gap-2">
                    <SeverityChip severity={finding.severity} />
                    {onFocus ? (
                        <button
                            className="min-w-0 truncate text-left font-semibold hover:text-accent-strong"
                            onClick={onFocus}
                            title={`Show ${location}`}
                            type="button"
                        >
                            {finding.title}
                        </button>
                    ) : (
                        <span className="min-w-0 truncate font-semibold">{finding.title}</span>
                    )}
                    <span className="flex-1" />
                    {!inline && (
                        <span className="shrink-0 truncate font-mono text-[11px] text-muted">
                            {location}
                        </span>
                    )}
                </header>
                <p className="selectable m-0 leading-normal whitespace-pre-wrap text-body">
                    {finding.body}
                </p>
                {finding.suggestion && (
                    <pre className="selectable m-0 overflow-x-auto rounded-md border border-line bg-app px-2.5 py-1.5 font-mono text-[12px] text-body">
                        {finding.suggestion}
                    </pre>
                )}
                <footer className="flex gap-1.5">
                    <Button
                        onClick={() =>
                            void navigator.clipboard.writeText(
                                `${location} — ${finding.title}\n\n${finding.body}${finding.suggestion ? `\n\n${finding.suggestion}` : ''}`,
                            )
                        }
                        size="sm"
                        variant="ghost"
                    >
                        Copy
                    </Button>
                    <Button onClick={onDismiss} size="sm" variant="ghost">
                        <Icon name="x" size={11} /> Dismiss
                    </Button>
                </footer>
            </article>
        </div>
    );
}

export function SeverityChip({ severity }: { severity: Severity }) {
    const style = severities[severity];

    return (
        <span
            className={`rounded-full px-2 py-0.5 font-sans text-[11px] font-semibold ${style.chip}`}
        >
            {style.label}
        </span>
    );
}
