const REPOSITORY = 'kevenleone/seam';

interface Release {
    assets: ReleaseAsset[];
    tag_name: string;
}

interface ReleaseAsset {
    browser_download_url: string;
    name: string;
}

document.documentElement.classList.add('js');

/** Points the download buttons at the latest .dmg; the static releases link stays if GitHub is unreachable. */
async function linkLatestRelease(): Promise<void> {
    const response = await fetch(`https://api.github.com/repos/${REPOSITORY}/releases/latest`, {
        headers: { Accept: 'application/vnd.github+json' },
    });

    if (!response.ok) {
        return;
    }

    const release = (await response.json()) as Release;
    const dmg = release.assets.find((asset) => asset.name.endsWith('.dmg'));

    if (!dmg) {
        return;
    }

    for (const link of document.querySelectorAll<HTMLAnchorElement>('[data-download]')) {
        link.href = dmg.browser_download_url;
    }

    const note = document.querySelector('[data-release-note]');

    if (note) {
        note.textContent = `Version ${release.tag_name.replace(/^v/, '')} for Apple silicon and Intel Macs.`;
    }
}

function revealOnScroll(): void {
    const observer = new IntersectionObserver(
        (entries) => {
            for (const entry of entries) {
                if (entry.isIntersecting) {
                    entry.target.classList.add('is-visible');
                    observer.unobserve(entry.target);
                }
            }
        },
        { rootMargin: '0px 0px -10% 0px', threshold: 0.1 },
    );

    for (const element of document.querySelectorAll('[data-reveal]')) {
        observer.observe(element);
    }
}

revealOnScroll();
linkLatestRelease().catch(() => undefined);
