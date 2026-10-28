import { asset } from '$app/paths'

export interface BundledImage {
  id: string
  title: string
  src: string
  /** Who made it, and where the copy comes from; all are in the public domain. */
  credit: string
  source: string
}

/** Public-domain images shipped with the site (static/images/som, see CREDITS.md there). */
export const BUNDLED: readonly BundledImage[] = [
  {
    id: 'great-wave',
    title: 'The Great Wave',
    src: asset('images/som/great-wave.jpg'),
    credit: 'Katsushika Hokusai, The Great Wave off Kanagawa, c. 1831',
    source: 'https://commons.wikimedia.org/wiki/File:Tsunami_by_hokusai_19th_century.jpg',
  },
  {
    id: 'starry-night',
    title: 'The Starry Night',
    src: asset('images/som/starry-night.jpg'),
    credit: 'Vincent van Gogh, The Starry Night, 1889 (Google Art Project)',
    source: 'https://commons.wikimedia.org/wiki/File:Van_Gogh_-_Starry_Night_-_Google_Art_Project.jpg',
  },
  {
    id: 'mona-lisa',
    title: 'Mona Lisa',
    src: asset('images/som/mona-lisa.jpg'),
    credit: 'Leonardo da Vinci, Mona Lisa, c. 1503–1519 (photograph: C2RMF)',
    source: 'https://commons.wikimedia.org/wiki/File:Mona_Lisa,_by_Leonardo_da_Vinci,_from_C2RMF_retouched.jpg',
  },
  {
    id: 'blue-marble',
    title: 'The Blue Marble',
    src: asset('images/som/blue-marble.jpg'),
    credit: 'NASA, The Blue Marble, Apollo 17, 1972',
    source: 'https://commons.wikimedia.org/wiki/File:The_Earth_seen_from_Apollo_17.jpg',
  },
]

/** Decodes an image from a URL or a picked file; rejects if it isn't one. */
export async function loadImage(source: string | File): Promise<HTMLImageElement> {
  const url = typeof source === 'string' ? source : URL.createObjectURL(source)
  try {
    const img = new Image()
    img.src = url
    await img.decode()
    if (img.naturalWidth === 0 || img.naturalHeight === 0) throw new Error('empty image')
    return img
  } finally {
    if (typeof source !== 'string') URL.revokeObjectURL(url)
  }
}

export interface Pixels {
  width: number
  height: number
  rgba: Uint8ClampedArray
}

/** The image's pixels, scaled down to at most `maxSide` on its longer side. */
export function toPixels(img: HTMLImageElement, maxSide = 256): Pixels {
  const scale = Math.min(1, maxSide / Math.max(img.naturalWidth, img.naturalHeight))
  const width = Math.max(1, Math.round(img.naturalWidth * scale))
  const height = Math.max(1, Math.round(img.naturalHeight * scale))
  const ctx = context(width, height)
  ctx.drawImage(img, 0, 0, width, height)
  return { width, height, rgba: ctx.getImageData(0, 0, width, height).data }
}

/**
 * The image as a density on a `side`×`side` grid: darkness (or lightness,
 * inverted) from 0 to 1, fitted into the square and centered; the margins
 * stay empty.
 */
export function toDensity(img: HTMLImageElement, side = 128, invert = false): Float32Array {
  const ctx = context(side, side)
  // A background that maps to no density either way.
  ctx.fillStyle = invert ? '#000' : '#fff'
  ctx.fillRect(0, 0, side, side)
  const scale = side / Math.max(img.naturalWidth, img.naturalHeight)
  const w = img.naturalWidth * scale
  const h = img.naturalHeight * scale
  ctx.drawImage(img, (side - w) / 2, (side - h) / 2, w, h)
  const px = ctx.getImageData(0, 0, side, side).data
  const cells = new Float32Array(side * side)
  for (let i = 0; i < cells.length; i++) {
    const lightness = (0.2126 * px[i * 4] + 0.7152 * px[i * 4 + 1] + 0.0722 * px[i * 4 + 2]) / 255
    cells[i] = invert ? lightness : 1 - lightness
  }
  return cells
}

function context(width: number, height: number): CanvasRenderingContext2D {
  const canvas = document.createElement('canvas')
  canvas.width = width
  canvas.height = height
  return canvas.getContext('2d', { willReadFrequently: true })!
}
