import { existsSync } from 'node:fs';
import { resolve } from 'node:path';

/** Intrinsic dimensions are required when registering final artwork. */
export interface BrandAsset {
  light: string;
  dark?: string;
  width: number;
  height: number;
  alt: string;
}

/** Approved artwork; logo and mark remain unregistered until supplied. */
export const brand: { logo?: BrandAsset; mark?: BrandAsset; concept?: BrandAsset } = {
  concept: {
    light: '/brand/cribra-concept.png',
    width: 1536,
    height: 1024,
    alt: 'Sensitive-data symbols and binary digits enter a Venetian mask from the left; structured output flows leave to the right.',
  },
};

/** Only build-time verified local brand assets produce image requests. */
export function availableAsset(asset?: BrandAsset): BrandAsset | undefined {
  if (!asset || asset.width <= 0 || asset.height <= 0) return undefined;
  const exists = (path: string) =>
    /^\/brand\/[\w.-]+$/.test(path) && existsSync(resolve('public', path.slice(1)));
  if (!exists(asset.light)) return undefined;
  return { ...asset, dark: asset.dark && exists(asset.dark) ? asset.dark : undefined };
}
