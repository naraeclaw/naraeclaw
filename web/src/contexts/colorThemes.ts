import themesData from './themes.json';

export type ColorThemeId =
  | 'default-dark' | 'default-light' | 'oled-black'
  | 'nord-dark' | 'nord-light'
  | 'dracula' | 'monokai'
  | 'solarized-dark' | 'solarized-light'
  | 'kanagawa-wave' | 'kanagawa-dragon' | 'kanagawa-lotus'
  | 'rose-pine' | 'rose-pine-moon' | 'rose-pine-dawn'
  | 'night-owl'
  | 'everforest-dark' | 'everforest-light'
  | 'cobalt2'
  | 'flexoki-dark' | 'flexoki-light'
  | 'hacker-green'
  | 'material-dark' | 'material-light';

export interface ColorThemeDef {
  id: ColorThemeId;
  name: string;
  scheme: 'dark' | 'light';
  preview: [string, string, string, string, string];
  vars: Record<string, string>;
}

export const colorThemes: ColorThemeDef[] = themesData as unknown as ColorThemeDef[];

export const colorThemeMap: Record<ColorThemeId, ColorThemeDef> =
  Object.fromEntries(colorThemes.map(t => [t.id, t])) as Record<ColorThemeId, ColorThemeDef>;

export const DEFAULT_DARK_THEME: ColorThemeId = 'kanagawa-wave';
export const DEFAULT_LIGHT_THEME: ColorThemeId = 'kanagawa-lotus';

// Kanagawa secondary palette — added to vars at apply time
export const KANAGAWA_EXTRA_VARS: Record<string, string> = {
  '--pc-iris':    '#957fb8',
  '--pc-spring':  '#98bb6c',
  '--pc-carp':    '#e6c384',
  '--pc-sakura':  '#d27e99',
  '--pc-wave':    '#7fb4ca',
  '--pc-autumn':  '#ffa066',
  '--pc-samurai': '#e82424',
};
