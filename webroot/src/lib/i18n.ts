/**
 * 轻量 i18n [Minimal i18n]
 *
 * 字典放在 public/i18n/{lang}.json，运行时按需加载；不引入 vue-i18n。
 * 缺少键时直接返回键名，便于发现漏翻。
 */

import { reactive } from 'vue';

export type Lang = 'zh' | 'en' | 'ja';
export type LangChoice = 'auto' | Lang;

const LANGS: Lang[] = ['zh', 'en', 'ja'];
const state = reactive({
  lang: 'zh' as Lang,
  dict: {} as Record<string, string>,
  ready: false,
});

const cache: Partial<Record<Lang, Record<string, string>>> = {};

export function detectLang(): Lang {
  const saved = localStorage.getItem('tf-lang');
  if (saved === 'en' || saved === 'zh' || saved === 'ja') return saved;
  const nav = (navigator.language || 'en').toLowerCase();
  if (nav.startsWith('zh')) return 'zh';
  if (nav.startsWith('ja')) return 'ja';
  return 'en';
}

export function savedLang(): LangChoice {
  const saved = localStorage.getItem('tf-lang');
  return saved === 'en' || saved === 'zh' || saved === 'ja' ? saved : 'auto';
}

async function load(lang: Lang): Promise<Record<string, string>> {
  if (cache[lang]) return cache[lang]!;
  try {
    const response = await fetch(`./i18n/${lang}.json`);
    if (!response.ok) throw new Error(String(response.status));
    const dict = (await response.json()) as Record<string, string>;
    cache[lang] = dict;
    return dict;
  } catch {
    return {};
  }
}

export async function setLanguage(choice: LangChoice): Promise<void> {
  if (choice === 'auto') localStorage.removeItem('tf-lang');
  else localStorage.setItem('tf-lang', choice);
  await apply();
}

export async function initI18n(): Promise<void> {
  await apply();
}

async function apply(): Promise<void> {
  const lang = detectLang();
  let dict = await load(lang);
  // 字典缺失时退回英文，避免整页显示键名。
  if (Object.keys(dict).length === 0 && lang !== 'en') dict = await load('en');
  state.lang = lang;
  state.dict = dict;
  state.ready = true;
  document.documentElement.lang = lang === 'zh' ? 'zh-CN' : lang;
}

export function currentLang(): Lang {
  return state.lang;
}

export function hasKey(key: string): boolean {
  return key in state.dict;
}

export function t(key: string, params?: Record<string, string | number>): string {
  let text = state.dict[key] ?? key;
  if (params) {
    for (const [name, value] of Object.entries(params)) {
      text = text.replace(new RegExp(`\\{${name}\\}`, 'g'), String(value));
    }
  }
  return text;
}

export { LANGS, state as i18nState };
