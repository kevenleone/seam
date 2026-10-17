import type { FileDiff } from './types';

const IMAGE_TYPES: Record<string, string> = {
    avif: 'image/avif',
    bmp: 'image/bmp',
    gif: 'image/gif',
    ico: 'image/x-icon',
    jpeg: 'image/jpeg',
    jpg: 'image/jpeg',
    png: 'image/png',
    webp: 'image/webp',
};

export interface BlobIds {
    after: null | string;
    before: null | string;
}

export function blobIds(diff: FileDiff): BlobIds {
    const match = diff.headerLines
        .map((headerLine) => /^index ([0-9a-f]+)\.\.([0-9a-f]+)/.exec(headerLine))
        .find(Boolean);
    const present = (object: string | undefined) =>
        object && !/^0+$/.test(object) ? object : null;

    return { after: present(match?.[2]), before: present(match?.[1]) };
}

export function imageType(path: string): null | string {
    const extension = path.slice(path.lastIndexOf('.') + 1).toLowerCase();

    return IMAGE_TYPES[extension] ?? null;
}
