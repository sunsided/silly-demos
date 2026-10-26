// Downloads the public-domain images the Kohonen lab offers as colour and
// density sources, scales them to 256 px with ImageMagick and writes them
// with their credits to web/static/images/som/.
//
// Run with `task data:images` (needs network access and ImageMagick). The
// output is committed; the site build never runs this.

import { execFileSync } from 'node:child_process'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const OUT = fileURLToPath(new URL('../static/images/som/', import.meta.url))
const SIZE = 256
// Wikimedia asks automated clients to say who they are.
const USER_AGENT = 'silly-demos image fetch (https://github.com/sunsided/silly-demos)'
const COMMONS = 'https://upload.wikimedia.org/wikipedia/commons/thumb'

const IMAGES = [
  {
    id: 'mona-lisa',
    title: 'Mona Lisa',
    credit: 'Leonardo da Vinci, c. 1503–1519; photograph by C2RMF',
    page: 'https://commons.wikimedia.org/wiki/File:Mona_Lisa,_by_Leonardo_da_Vinci,_from_C2RMF_retouched.jpg',
    url: `${COMMONS}/e/ec/Mona_Lisa%2C_by_Leonardo_da_Vinci%2C_from_C2RMF_retouched.jpg/1280px-Mona_Lisa%2C_by_Leonardo_da_Vinci%2C_from_C2RMF_retouched.jpg`,
  },
  {
    id: 'starry-night',
    title: 'The Starry Night',
    credit: 'Vincent van Gogh, 1889; Google Art Project',
    page: 'https://commons.wikimedia.org/wiki/File:Van_Gogh_-_Starry_Night_-_Google_Art_Project.jpg',
    url: `${COMMONS}/e/ea/Van_Gogh_-_Starry_Night_-_Google_Art_Project.jpg/1280px-Van_Gogh_-_Starry_Night_-_Google_Art_Project.jpg`,
  },
  {
    id: 'great-wave',
    title: 'The Great Wave off Kanagawa',
    credit: 'Katsushika Hokusai, c. 1831',
    page: 'https://commons.wikimedia.org/wiki/File:Tsunami_by_hokusai_19th_century.jpg',
    url: `${COMMONS}/a/a5/Tsunami_by_hokusai_19th_century.jpg/1280px-Tsunami_by_hokusai_19th_century.jpg`,
  },
  {
    id: 'blue-marble',
    title: 'The Blue Marble',
    credit: 'NASA, Apollo 17 crew, 1972',
    page: 'https://commons.wikimedia.org/wiki/File:The_Earth_seen_from_Apollo_17.jpg',
    url: `${COMMONS}/9/97/The_Earth_seen_from_Apollo_17.jpg/1280px-The_Earth_seen_from_Apollo_17.jpg`,
  },
]

/** ImageMagick 7 is `magick`; 6 is `convert`. */
function imagemagick() {
  for (const cmd of ['magick', 'convert']) {
    try {
      execFileSync(cmd, ['-version'], { stdio: 'ignore' })
      return cmd
    } catch {}
  }
  throw new Error('ImageMagick not found (magick or convert)')
}

const convert = imagemagick()
await mkdir(OUT, { recursive: true })
const tmp = await mkdtemp(join(tmpdir(), 'som-images-'))
try {
  for (const img of IMAGES) {
    const res = await fetch(img.url, { headers: { 'User-Agent': USER_AGENT } })
    if (!res.ok) throw new Error(`${img.id}: HTTP ${res.status}`)
    const original = join(tmp, `${img.id}.orig`)
    await writeFile(original, Buffer.from(await res.arrayBuffer()))
    const out = join(OUT, `${img.id}.jpg`)
    execFileSync(convert, [original, '-resize', `${SIZE}x${SIZE}>`, '-strip', '-quality', '85', out])
    console.log(`${img.id}: ${out}`)
  }
} finally {
  await rm(tmp, { recursive: true, force: true })
}

const credits = [
  '# Image credits',
  '',
  'All images are in the public domain. They are scaled to 256 px for the Kohonen lab.',
  '',
  ...IMAGES.map(img => `- \`${img.id}.jpg\`: *${img.title}*, ${img.credit}. Public domain. Source: <${img.page}>`),
  '',
]
await writeFile(join(OUT, 'CREDITS.md'), credits.join('\n'))
console.log(`wrote ${join(OUT, 'CREDITS.md')}`)
