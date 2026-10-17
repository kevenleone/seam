const paths = {
    branch: 'M6 3v12M18 9a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM18 9a9 9 0 0 1-9 9',
    check: 'M5 12l5 5 9-10',
    chevronDown: 'M6 9l6 6 6-6',
    chevronRight: 'M9 6l6 6-6 6',
    command: 'M9 6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3z',
    down: 'M12 5v14M19 12l-7 7-7-7',
    download: 'M12 3v12M7 10l5 5 5-5M5 21h14',
    external: 'M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5',
    folder: 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z',
    gear: 'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z',
    merge: 'M6 3v18M18 15a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM6 6a9 9 0 0 0 9 9h3',
    minus: 'M5 12h14',
    plus: 'M12 5v14M5 12h14',
    refresh: 'M21 12a9 9 0 1 1-3-6.7L21 8M21 3v5h-5',
    search: 'M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16zM21 21l-4.3-4.3',
    sidebarLeft: 'M4 4h16v16H4zM9 4v16',
    sidebarRight: 'M4 4h16v16H4zM15 4v16',
    tag: 'M3 12V3h9l9 9-9 9zM7.5 7.5h.01',
    tray: 'M4 13l2-8h12l2 8M4 13v6h16v-6M4 13h5l1 2h4l1-2h5',
    undo: 'M9 14L4 9l5-5M4 9h11a5 5 0 0 1 0 10h-3',
    up: 'M12 19V5M5 12l7-7 7 7',
    warning: 'M12 3l9 16H3zM12 10v4M12 17h.01',
    x: 'M6 6l12 12M18 6L6 18',
} as const;

export type IconName = keyof typeof paths;

export function Icon({
    className,
    name,
    size = 14,
    strokeWidth = 2,
}: {
    className?: string;
    name: IconName;
    size?: number;
    strokeWidth?: number;
}) {
    return (
        <svg
            aria-hidden="true"
            className={className}
            fill="none"
            height={size}
            stroke="currentColor"
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={strokeWidth}
            viewBox="0 0 24 24"
            width={size}
        >
            <path d={paths[name]} />
        </svg>
    );
}

/** The Seam mark: two panes of a diff joined by lavender stitches. */
export function SeamMark({ size = 48 }: { size?: number }) {
    return (
        <svg aria-label="Seam" height={size} role="img" viewBox="0 0 100 100" width={size}>
            <rect fill="#16171C" height="100" rx="22" stroke="#33353F" width="100" />
            <rect fill="#183154" height="60" rx="6" width="29" x="16" y="20" />
            <rect fill="#3E2515" height="60" rx="6" width="29" x="55" y="20" />
            <rect fill="#6CB6FF" height="4" rx="2" width="16" x="22" y="30" />
            <rect fill="#6CB6FF" height="4" rx="2" width="12" x="22" y="40" />
            <rect fill="#F0A060" height="4" rx="2" width="16" x="62" y="56" />
            <rect fill="#F0A060" height="4" rx="2" width="10" x="62" y="66" />
            <path
                d="M42 30l16 7M42 44l16 7M42 58l16 7"
                stroke="#B9A8FF"
                strokeLinecap="round"
                strokeWidth="4.5"
            />
        </svg>
    );
}
