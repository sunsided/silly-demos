import { MediaQuery } from 'svelte/reactivity'
import { palette, tokens, type Palette } from '../tokens/generated/tokens.ts'

/** Colors and font for canvas drawing, from the same tokens as the CSS. */
export type Theme = Palette & { font: string; labelSize: number }

const dark = new MediaQuery('prefers-color-scheme: dark')

const themes = {
  light: { ...palette.light, font: tokens.font.family.sans, labelSize: tokens.font.canvasLabel },
  dark: { ...palette.dark, font: tokens.font.family.sans, labelSize: tokens.font.canvasLabel },
} satisfies Record<string, Theme>

/** The theme for the current color scheme; cheap enough to call every frame. */
export function currentTheme(): Theme {
  return dark.current ? themes.dark : themes.light
}

/** Whether the dark color scheme is in use; reactive. */
export function darkScheme(): boolean {
  return dark.current
}
