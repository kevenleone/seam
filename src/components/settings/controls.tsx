// The pieces every settings pane is built from, after Lore's: section labels,
// title/description rows with a trailing control, segmented controls and the switch.

import type { ReactNode } from 'react';

export const TEXT_FIELD =
    'h-[30px] w-[230px] rounded-[7px] border border-control-line bg-app px-2.5 font-mono text-[12px] outline-none focus:border-accent';

export const SELECT =
    'h-[30px] rounded-[7px] border border-control-line bg-control px-2 text-fg outline-none focus:border-accent';

/** A setting: its title, an optional description, and the control that changes it. */
export function Row({
    children,
    desc,
    last,
    title,
}: {
    children?: ReactNode;
    desc?: ReactNode;
    last?: boolean;
    title: string;
}) {
    return (
        <div className={`flex items-start gap-4 py-3.5 ${last ? '' : 'border-b border-line'}`}>
            <div className="min-w-0 flex-1">
                <div className="font-semibold">{title}</div>
                {desc && (
                    <div className="mt-0.5 text-[12.5px] leading-normal text-muted">{desc}</div>
                )}
            </div>
            {children && <div className="flex flex-none items-center gap-2">{children}</div>}
        </div>
    );
}

export function SectionLabel({ children, first }: { children: ReactNode; first?: boolean }) {
    return (
        <h3
            className={`mb-1 text-[11px] font-semibold tracking-[0.08em] text-dim uppercase ${first ? 'mt-0' : 'mt-7'}`}
        >
            {children}
        </h3>
    );
}

export function Segmented<TValue extends string>({
    label,
    onChange,
    options,
    value,
}: {
    label: string;
    onChange(value: TValue): void;
    options: readonly { label: string; value: TValue }[];
    value: TValue;
}) {
    return (
        <div
            aria-label={label}
            className="flex flex-none gap-0.5 rounded-lg bg-control p-[3px]"
            role="radiogroup"
        >
            {options.map((option) => (
                <button
                    aria-checked={option.value === value}
                    className={`rounded-md px-3 py-1 text-[12.5px] ${option.value === value ? 'bg-tab font-semibold text-fg shadow-[0_1px_2px_var(--shadow)]' : 'text-muted hover:text-fg'}`}
                    key={option.value}
                    onClick={() => onChange(option.value)}
                    role="radio"
                    type="button"
                >
                    {option.label}
                </button>
            ))}
        </div>
    );
}

/** The 38×22 pill switch. */
export function Toggle({
    label,
    on,
    onChange,
}: {
    label: string;
    on: boolean;
    onChange(next: boolean): void;
}) {
    return (
        <button
            aria-checked={on}
            aria-label={label}
            className={`relative h-[22px] w-[38px] flex-none rounded-full transition-colors duration-150 ${on ? 'bg-accent' : 'bg-control-line'}`}
            onClick={() => onChange(!on)}
            role="switch"
            type="button"
        >
            <span
                className={`absolute top-[2px] h-[18px] w-[18px] rounded-full bg-white shadow-[0_1px_3px_rgba(0,0,0,0.25)] transition-[left] duration-150 ${on ? 'left-[18px]' : 'left-[2px]'}`}
            />
        </button>
    );
}
