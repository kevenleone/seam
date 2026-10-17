import { type ButtonHTMLAttributes, type ReactNode, useEffect, useRef } from 'react';

type Variant = 'add' | 'danger' | 'default' | 'del' | 'ghost' | 'primary';

const variants: Record<Variant, string> = {
    add: 'border border-add-line text-add-num hover:bg-add-bg',
    danger: 'border border-del-line text-warn hover:bg-warn-bg',
    default: 'border border-control-line bg-transparent hover:bg-hover',
    del: 'border border-del-line text-warn hover:bg-del-bg',
    ghost: 'border-0 bg-transparent text-muted hover:text-fg hover:bg-hover',
    primary: 'border-0 bg-accent text-on-accent font-semibold hover:brightness-110',
};

export function Button({
    className = '',
    size = 'md',
    variant = 'default',
    ...props
}: { size?: 'lg' | 'md' | 'sm'; variant?: Variant } & ButtonHTMLAttributes<HTMLButtonElement>) {
    const sizes = {
        lg: 'h-[38px] px-4 rounded-lg',
        md: 'h-[30px] px-3 rounded-[7px]',
        sm: 'h-[26px] px-2.5 text-[12px] rounded-md',
    };

    return (
        <button
            type="button"
            {...props}
            className={`inline-flex items-center justify-center gap-1.5 whitespace-nowrap transition-colors disabled:cursor-not-allowed disabled:opacity-45 ${sizes[size]} ${variants[variant]} ${className}`}
        />
    );
}

export function Checkbox({
    checked,
    label,
    onChange,
}: {
    checked: boolean;
    label: ReactNode;
    onChange: (v: boolean) => void;
}) {
    return (
        <label className="flex cursor-pointer items-center gap-2">
            <input
                checked={checked}
                className="m-0 accent-[var(--accent)]"
                onChange={(event) => onChange(event.target.checked)}
                type="checkbox"
            />
            {label}
        </label>
    );
}

/**
 * A side panel that slides open and closed by animating its width. The content keeps its
 * full width while the outer box narrows, so nothing reflows mid-animation. A closed panel
 * is `inert`, so Tab and screen readers skip it.
 */
export function Collapsible({
    children,
    open,
    width,
}: {
    children: ReactNode;
    open: boolean;
    width: number;
}) {
    return (
        <div
            className="h-full shrink-0 overflow-hidden transition-[width] duration-200 ease-out motion-reduce:transition-none"
            inert={!open}
            style={{ width: open ? width : 0 }}
        >
            <div
                className={`h-full transition-opacity duration-200 motion-reduce:transition-none ${open ? 'opacity-100' : 'opacity-0'}`}
                style={{ width }}
            >
                {children}
            </div>
        </div>
    );
}

export function Kbd({ children }: { children: ReactNode }) {
    return <span className="font-mono text-[11.5px] text-muted">{children}</span>;
}

/** A centred dialog over a dimmed window. Escape and the backdrop close it. */
export function Modal({
    children,
    onClose,
    title,
    width = 460,
}: {
    children: ReactNode;
    onClose: () => void;
    title: string;
    width?: number;
}) {
    const ref = useRef<HTMLDivElement>(null);

    useEffect(() => {
        const first = ref.current?.querySelector<HTMLElement>('input, textarea, select, button');

        first?.focus();

        const onKey = (event: KeyboardEvent) => {
            if (event.key === 'Escape') {
                event.stopPropagation();
                onClose();
            }
        };

        window.addEventListener('keydown', onKey, true);

        return () => window.removeEventListener('keydown', onKey, true);
    }, [onClose]);

    return (
        <div
            className="fixed inset-0 z-40 flex items-start justify-center bg-black/40 pt-[12vh]"
            onMouseDown={(event) => event.target === event.currentTarget && onClose()}
        >
            <div
                aria-label={title}
                aria-modal="true"
                className="flex max-w-[92vw] flex-col gap-3.5 rounded-xl border border-control-line bg-panel p-5 shadow-[0_20px_60px_var(--shadow)]"
                ref={ref}
                role="dialog"
                style={{ width }}
            >
                <h2 className="m-0 text-[15px] font-semibold">{title}</h2>
                {children}
            </div>
        </div>
    );
}

export function SectionLabel({
    children,
    className = '',
}: {
    children: ReactNode;
    className?: string;
}) {
    return (
        <h2
            className={`m-0 text-[11px] font-semibold tracking-[0.08em] text-muted uppercase ${className}`}
        >
            {children}
        </h2>
    );
}

export function TextArea(props: React.TextareaHTMLAttributes<HTMLTextAreaElement>) {
    return (
        <textarea
            {...props}
            className={`resize-y rounded-[7px] border border-control-line bg-app px-2.5 py-2 leading-normal outline-none focus:border-accent ${props.className ?? ''}`}
        />
    );
}

export function TextField(props: React.InputHTMLAttributes<HTMLInputElement>) {
    return (
        <input
            {...props}
            className={`h-[32px] rounded-[7px] border border-control-line bg-app px-2.5 outline-none focus:border-accent ${props.className ?? ''}`}
        />
    );
}

const rtf = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });

export function relativeTime(seconds: number): string {
    const diff = seconds - Date.now() / 1000;
    const units: [Intl.RelativeTimeFormatUnit, number][] = [
        ['year', 31536000],
        ['month', 2592000],
        ['week', 604800],
        ['day', 86400],
        ['hour', 3600],
        ['minute', 60],
    ];

    for (const [unit, size] of units) {
        if (Math.abs(diff) >= size) {
            return rtf.format(Math.round(diff / size), unit);
        }
    }

    return 'just now';
}
