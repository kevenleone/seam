import type { ThemeDefinition } from '../../lib/themes';

import { useSettings } from '../../lib/settings';
import { accentById, applyTokens } from '../../lib/themes';

const ROW = 'h-[3px] rounded-full bg-dim opacity-60';

/**
 * A miniature Seam window painted with the theme's own tokens (sidebar, a hunk with an
 * addition and a deletion), so the card shows what the theme does to a diff.
 */
export function ThemePreview({ theme }: { theme: ThemeDefinition }) {
    const { accent } = useSettings();

    return (
        <span
            className="block aspect-[4/3] overflow-hidden rounded-lg border border-line bg-app"
            ref={(element) => {
                if (element) {
                    applyTokens(element, theme, accentById(accent));
                }
            }}
        >
            <span className="flex h-full flex-col">
                <span className="flex h-[10px] flex-none items-center gap-[3px] border-b border-line bg-panel px-[5px]">
                    <span className="h-[3px] w-[3px] rounded-full bg-del" />
                    <span className="h-[3px] w-[3px] rounded-full bg-warn" />
                    <span className="h-[3px] w-[3px] rounded-full bg-add" />
                    <span className="ml-auto h-[3px] w-[14px] rounded-full bg-accent opacity-80" />
                </span>
                <span className="flex min-h-0 flex-1">
                    <span className="flex w-[30%] flex-none flex-col gap-[4px] border-r border-line bg-panel p-[5px]">
                        <span className={`${ROW} w-[80%]`} />
                        <span className={`${ROW} w-[62%]`} />
                        <span className="h-[3px] w-[70%] rounded-full bg-accent opacity-70" />
                        <span className={`${ROW} w-[52%]`} />
                    </span>
                    <span className="flex min-w-0 flex-1 flex-col gap-[3px] bg-app py-[6px]">
                        <span className="mx-[6px] mb-[2px] h-[4px] w-[50%] rounded-full bg-fg opacity-80" />
                        <span className="h-[5px] bg-hunk" />
                        <span className={`mx-[6px] ${ROW} w-[70%]`} />
                        <span className="flex h-[6px] items-center bg-del-bg px-[6px]">
                            <span className="h-[2px] w-[60%] rounded-full bg-del" />
                        </span>
                        <span className="flex h-[6px] items-center bg-add-bg px-[6px]">
                            <span className="h-[2px] w-[76%] rounded-full bg-add" />
                        </span>
                        <span className="flex h-[6px] items-center bg-add-bg px-[6px]">
                            <span className="h-[2px] w-[44%] rounded-full bg-add" />
                        </span>
                        <span className={`mx-[6px] ${ROW} w-[64%]`} />
                    </span>
                </span>
            </span>
        </span>
    );
}
