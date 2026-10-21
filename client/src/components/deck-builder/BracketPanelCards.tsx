import { Fragment } from "react";
import { useTranslation } from "react-i18next";

import type { BracketCard } from "../../types/bracket";

interface CardNameProps {
  card: BracketCard;
  onCardClick: (name: string) => void;
}

export function BracketPanelCardName({ card, onCardClick }: CardNameProps) {
  const { t } = useTranslation("deck-builder");

  return (
    <span className="inline-flex items-center gap-1">
      <button
        type="button"
        onClick={() => onCardClick(card.name)}
        className="inline-flex min-h-[44px] items-center text-slate-300 underline-offset-2 hover:underline sm:min-h-0"
      >
        {card.name}
      </button>
      {card.role === "commander" && (
        <span className="rounded-full border border-white/10 px-1.5 text-[10px] text-slate-400">
          {t("bracket.card.commanderTag")}
        </span>
      )}
    </span>
  );
}

interface CardListProps {
  cards: BracketCard[];
  joiner: "comma" | "or";
  onCardClick: (name: string) => void;
}

const JOINER_TEXT: Record<CardListProps["joiner"], string> = {
  comma: ", ",
  or: " or ",
};

export function BracketPanelCardList({ cards, joiner, onCardClick }: CardListProps) {
  return (
    <span className="inline-flex flex-wrap items-center">
      {cards.map((card, index) => (
        <Fragment key={`${card.role}-${card.name}`}>
          {index > 0 && JOINER_TEXT[joiner]}
          <BracketPanelCardName card={card} onCardClick={onCardClick} />
        </Fragment>
      ))}
    </span>
  );
}
