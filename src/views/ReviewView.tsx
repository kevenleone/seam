import { useMemo } from 'react';

import type { Finding, Review, Severity } from '../lib/types';

import { RunBar } from '../components/ChangeOverview';
import { DiffView } from '../components/DiffView';
import { ExpandButton } from '../components/ExpandButton';
import { FindingCard } from '../components/FindingCard';
import { Icon } from '../components/Icon';
import { ProviderSelect } from '../components/ProviderSelect';
import { ReviewEmpty, ReviewRunStatus, WhileReviewing } from '../components/ReviewStates';
import { Button, Collapsible, relativeTime, SectionLabel } from '../components/ui';
import { PROVIDER_LABELS } from '../lib/ai';
import { useSettings } from '../lib/settings';
import { useSettingsModal } from '../lib/settingsModal';
import { useRepo } from '../lib/store';

const order: Severity[] = ['risk', 'check', 'note'];

export function ReviewView() {
    const { leftPanel, rightPanel } = useSettings();
    const { dismissed, expanded, review } = useRepo();
    const shown =
        review.status === 'done'
            ? review.review
            : review.status === 'failed'
              ? review.previous
              : null;
    const findings = useMemo(
        () => shown?.findings.filter((finding) => !dismissed.includes(finding.id)) ?? [],
        [shown, dismissed],
    );

    return (
        <>
            <Collapsible open={leftPanel && !expanded && !!shown} width={264}>
                {shown && <ReviewFiles findings={findings} review={shown} />}
            </Collapsible>
            <section aria-label="Review" className="flex min-w-0 flex-1 flex-col">
                <ReviewHeader />
                {review.status === 'running' && <RunBar />}
                <ReviewBody findings={findings} shown={shown} />
            </section>
            <Collapsible open={rightPanel && !expanded && !!shown} width={360}>
                {shown && <FindingsPanel findings={findings} review={shown} />}
            </Collapsible>
        </>
    );
}

function FindingsPanel({ findings, review }: { findings: Finding[]; review: Review }) {
    const { dismissFinding, focusFinding } = useRepo();
    const counts = order.map(
        (severity) =>
            [severity, findings.filter((finding) => finding.severity === severity).length] as const,
    );

    return (
        <aside
            aria-label="Findings"
            className="flex h-full w-[360px] flex-col gap-3.5 overflow-auto border-l border-line bg-panel p-4"
        >
            <div className="flex flex-col gap-1.5">
                <SectionLabel>Summary</SectionLabel>
                <p className="selectable m-0 leading-normal text-body">
                    {review.summary || 'No summary.'}
                </p>
                <span className="text-[12px] text-muted">
                    {counts
                        .filter(([, count]) => count > 0)
                        .map(([severity, count]) => `${count} ${severity}`)
                        .join(' · ') || 'No findings'}
                </span>
            </div>
            {order.map((severity) => {
                const group = findings.filter((finding) => finding.severity === severity);

                return (
                    group.length > 0 && (
                        <section className="flex flex-col gap-2" key={severity}>
                            {group.map((finding) => (
                                <FindingCard
                                    finding={finding}
                                    key={finding.id}
                                    onDismiss={() => dismissFinding(finding.id)}
                                    onFocus={() => focusFinding(finding)}
                                />
                            ))}
                        </section>
                    )
                );
            })}
            {findings.length === 0 && review.findings.length > 0 && (
                <span className="text-[12px] text-muted">All findings dismissed.</span>
            )}
        </aside>
    );
}

function ReviewBody({ findings, shown }: { findings: Finding[]; shown: null | Review }) {
    const { dismissFinding, review, reviewFocus, startReview } = useRepo();

    if (review.status === 'running') {
        return <WhileReviewing state={review} />;
    }

    const failure =
        review.status === 'failed' ? (
            <div
                className="flex items-start gap-2.5 border-b border-del-line bg-del-bg px-[18px] py-2.5"
                role="alert"
            >
                <Icon className="mt-0.5 shrink-0 text-del" name="warning" size={14} />
                <pre className="selectable m-0 flex-1 font-sans whitespace-pre-wrap text-body">
                    {review.message}
                </pre>
                <Button
                    onClick={() => useSettingsModal.getState().show('ai')}
                    size="sm"
                    variant="ghost"
                >
                    AI settings
                </Button>
                <Button onClick={() => startReview(review.provider)} size="sm">
                    Try again
                </Button>
            </div>
        ) : null;

    if (!shown) {
        return (
            <>
                {failure}
                <div className="min-h-0 flex-1 overflow-auto">
                    <ReviewEmpty />
                </div>
            </>
        );
    }

    return (
        <>
            {failure}
            {shown.truncated && (
                <div className="border-b border-line bg-warn-bg px-[18px] py-2 text-[12px] text-warn">
                    This change was too large to send whole; the model saw the first part of the
                    diff.
                </div>
            )}
            {shown.diffs.length ? (
                <DiffView
                    comments={{ findings, focus: reviewFocus, onDismiss: dismissFinding }}
                    diffs={shown.diffs}
                />
            ) : (
                <div className="flex flex-1 items-center justify-center text-muted">
                    No file changes in this review.
                </div>
            )}
        </>
    );
}

function ReviewFiles({ findings, review }: { findings: Finding[]; review: Review }) {
    const { set } = useRepo();

    return (
        <nav
            aria-label="Reviewed files"
            className="flex h-full w-[264px] flex-col gap-0.5 overflow-auto border-r border-line bg-panel px-2 py-3.5"
        >
            <SectionLabel className="px-2 pb-1.5">Files · {review.diffs.length}</SectionLabel>
            {review.diffs.map((diff) => {
                const path = diff.newPath ?? diff.oldPath ?? '';
                const slash = path.lastIndexOf('/');
                const count = findings.filter((finding) => finding.file === path).length;

                return (
                    <button
                        className="flex w-full items-center gap-2 rounded-md px-2 py-[7px] text-left hover:bg-hover"
                        key={path}
                        onClick={() =>
                            set({ reviewFocus: { file: path, line: null, nonce: Date.now() } })
                        }
                        type="button"
                    >
                        <span className="min-w-0 flex-1">
                            <span className="block truncate">{path.slice(slash + 1)}</span>
                            {slash > 0 && (
                                <span className="block truncate text-[11.5px] text-muted" dir="rtl">
                                    {path.slice(0, slash + 1)}
                                </span>
                            )}
                        </span>
                        {count > 0 && (
                            <span className="rounded-full bg-accent-bg px-1.5 font-mono text-[11px] text-accent">
                                {count}
                            </span>
                        )}
                    </button>
                );
            })}
        </nav>
    );
}

function ReviewHeader() {
    const { review, startReview, status } = useRepo();
    const shown = review.status === 'done' ? review.review : null;
    const running = review.status === 'running';
    const stale = shown?.headSha && status.branch.oid && shown.headSha !== status.branch.oid;

    return (
        <div className="flex shrink-0 flex-wrap items-center gap-2.5 border-b border-line px-[18px] py-2.5">
            {review.status === 'running' ? (
                <ReviewRunStatus state={review} />
            ) : shown ? (
                <>
                    <span className="font-mono text-[12.5px]">{shown.scope}</span>
                    <span className="text-[12px] text-muted">
                        {shown.commits > 0 &&
                            `${shown.commits} commit${shown.commits === 1 ? '' : 's'} · `}
                        {shown.diffs.length} file{shown.diffs.length === 1 ? '' : 's'} · reviewed{' '}
                        {relativeTime(shown.createdAt)} by {PROVIDER_LABELS[shown.provider]}
                    </span>
                    {stale && (
                        <span className="rounded-full bg-warn-bg px-2 py-0.5 text-[11.5px] text-warn">
                            New commits since this review
                        </span>
                    )}
                </>
            ) : (
                <span className="font-semibold">AI review</span>
            )}
            <span className="flex-1" />
            <ProviderSelect disabled={running} />
            <Button
                aria-label="AI settings"
                onClick={() => useSettingsModal.getState().show('ai')}
                size="sm"
                title="AI settings"
                variant="ghost"
            >
                <Icon name="gear" size={13} />
            </Button>
            {shown && !running && (
                <Button onClick={() => startReview()} size="sm">
                    <Icon name="refresh" size={12} /> Re-run review
                </Button>
            )}
            {shown && <ExpandButton />}
        </div>
    );
}
