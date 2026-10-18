import { type ReactNode, useEffect, useMemo, useRef, useState } from 'react';

import { Icon, type IconName } from './Icon';
import { score } from './Palette';

export interface PickItem {
    description?: ReactNode;
    detail?: string;
    /** Items sharing a group sit together; the first one shows its name on the right. */
    group?: string;
    icon: IconName;
    id: string;
    label: string;
    run(): void;
    /** Text matched besides the label and detail, e.g. what `description` says. */
    terms?: string;
}

/** A searchable list dropped from the top of the window, after VS Code's quick pick. */
export function QuickPick({
    items,
    onClose,
    placeholder,
}: {
    items: PickItem[];
    onClose(): void;
    placeholder: string;
}) {
    const [query, setQuery] = useState('');
    const [highlighted, setHighlighted] = useState(0);
    const listRef = useRef<HTMLDivElement>(null);
    const results = useMemo(() => {
        if (!query.trim()) {
            return items;
        }

        return items
            .map(
                (item) =>
                    [
                        item,
                        score(query, `${item.label} ${item.detail ?? ''} ${item.terms ?? ''}`),
                    ] as const,
            )
            .filter((entry): entry is readonly [PickItem, number] => entry[1] !== null)
            .sort((left, right) => left[1] - right[1])
            .map(([item]) => item);
    }, [items, query]);

    useEffect(() => setHighlighted(0), [query, items]);
    useEffect(() => {
        listRef.current
            ?.querySelector(`[data-index="${highlighted}"]`)
            ?.scrollIntoView({ block: 'nearest' });
    }, [highlighted]);

    const run = (index: number) => {
        const item = results[index];

        if (item) {
            onClose();
            item.run();
        }
    };

    return (
        <div
            className="fixed inset-0 z-40 flex items-start justify-center pt-[52px]"
            onMouseDown={(event) => event.target === event.currentTarget && onClose()}
        >
            <div
                aria-label={placeholder}
                className="flex w-[640px] max-w-[92vw] flex-col overflow-hidden rounded-lg border border-control-line bg-panel shadow-[0_18px_50px_var(--shadow)]"
                role="dialog"
            >
                <div className="p-1.5">
                    <input
                        aria-label={placeholder}
                        autoFocus
                        className="w-full rounded-md border border-accent-line bg-app px-2.5 py-1.5 text-[13px] outline-none"
                        onChange={(event) => setQuery(event.target.value)}
                        onKeyDown={(event) => {
                            if (event.key === 'ArrowDown') {
                                event.preventDefault();
                                setHighlighted((current) =>
                                    Math.min(current + 1, results.length - 1),
                                );
                            } else if (event.key === 'ArrowUp') {
                                event.preventDefault();
                                setHighlighted((current) => Math.max(current - 1, 0));
                            } else if (event.key === 'Enter') {
                                event.preventDefault();
                                run(highlighted);
                            } else if (event.key === 'Escape') {
                                event.preventDefault();
                                onClose();
                            }
                        }}
                        placeholder={placeholder}
                        value={query}
                    />
                </div>
                <div
                    className="max-h-[420px] overflow-auto px-1.5 pb-1.5"
                    ref={listRef}
                    role="listbox"
                >
                    {results.map((item, index) => {
                        const opensGroup = index > 0 && results[index - 1].group !== item.group;
                        const showsGroup = index === 0 || opensGroup;

                        return (
                            <button
                                aria-selected={index === highlighted}
                                className={`flex w-full flex-col gap-0.5 rounded-md px-2 py-1 text-left ${opensGroup ? 'mt-1 border-t border-line pt-1.5' : ''} ${index === highlighted ? 'bg-accent-bg' : ''}`}
                                data-index={index}
                                key={item.id}
                                onClick={() => run(index)}
                                onMouseMove={() => setHighlighted(index)}
                                role="option"
                                type="button"
                            >
                                <span className="flex w-full min-w-0 items-center gap-2">
                                    <Icon
                                        className="shrink-0 text-muted"
                                        name={item.icon}
                                        size={13}
                                    />
                                    <span className="truncate">{item.label}</span>
                                    {item.description && (
                                        <span className="shrink-0 text-[12px] text-muted">
                                            {item.description}
                                        </span>
                                    )}
                                    <span className="flex-1" />
                                    {showsGroup && item.group && (
                                        <span className="shrink-0 text-[11.5px] text-accent">
                                            {item.group}
                                        </span>
                                    )}
                                </span>
                                {item.detail && (
                                    <span className="truncate pl-[21px] text-[12px] text-muted">
                                        {item.detail}
                                    </span>
                                )}
                            </button>
                        );
                    })}
                    {!results.length && <div className="px-2 py-2 text-muted">Nothing matches</div>}
                </div>
            </div>
        </div>
    );
}
