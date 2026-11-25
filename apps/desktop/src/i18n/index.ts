import { enUS } from "./en-US";
import { ptBR } from "./pt-BR";
import type { Locale, TranslationDictionary } from "./types";

export const dictionaries: Record<Locale, TranslationDictionary> = {
  "pt-BR": ptBR,
  "en-US": enUS,
};

export function getDictionary(locale: Locale): TranslationDictionary {
  return dictionaries[locale] ?? ptBR;
}

export type { Locale, EffectKind, TranslationDictionary } from "./types";
