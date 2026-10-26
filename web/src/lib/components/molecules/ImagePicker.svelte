<!--
  @component
  Thumbnails of the bundled images and an upload button. Uploaded files are
  only decoded in the browser.
-->
<script lang="ts">
  import { BUNDLED, loadImage } from '../../demos/kohonen/image.ts'

  let {
    selected,
    onpick,
    onerror,
  }: {
    /** The id of the image in use: a bundled one's, or 'upload'. */
    selected: string
    onpick: (img: HTMLImageElement, id: string) => void
    onerror: (message: string) => void
  } = $props()

  async function pick(source: string | File, id: string) {
    try {
      onpick(await loadImage(source), id)
    } catch {
      onerror("Couldn't read that image")
    }
  }

  function upload(e: Event & { currentTarget: HTMLInputElement }) {
    const file = e.currentTarget.files?.[0]
    if (file) pick(file, 'upload')
    e.currentTarget.value = ''
  }
</script>

<div class="picker">
  {#each BUNDLED as img (img.id)}
    <button
      type="button"
      class:selected={selected === img.id}
      title={img.title}
      aria-label={img.title}
      aria-pressed={selected === img.id}
      onclick={() => pick(img.src, img.id)}
    >
      <img src={img.src} alt="" />
    </button>
  {/each}
  <label class="upload" class:selected={selected === 'upload'}>
    <input type="file" accept="image/*" onchange={upload} />
    Upload…
  </label>
</div>

<style>
  .picker {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: var(--space-xs);
  }

  button,
  .upload {
    aspect-ratio: 1;
    padding: 0;
    border: var(--border-hairline) solid var(--color-ink-faint);
    border-radius: var(--border-radius);
    background: var(--color-paper-raised);
    cursor: pointer;
    overflow: hidden;
  }

  .selected {
    border-color: var(--color-cobalt);
    outline: var(--border-hairline) solid var(--color-cobalt);
  }

  img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .upload {
    position: relative;
    display: grid;
    place-items: center;
    font-size: var(--font-size-xs);
    color: var(--color-ink-soft);
    text-align: center;
  }

  .upload:focus-within {
    outline: 2px solid var(--color-cobalt);
  }

  input {
    position: absolute;
    width: 1px;
    height: 1px;
    opacity: 0;
  }
</style>
