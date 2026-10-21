//! Explains a finished [`BracketEstimate`]: the reasons that hold the deck at its
//! estimated tier ("Why") and a ladder of steps down to the base floor ("Bring it
//! down"), each reason carrying the typed cuts that clear it.
//!
//! The deck builder's bracket panel renders this verbatim. Every grouping, count and
//! cut is decided here, because the frontend is a display layer.
//!
//! Steps describe the deck as it is. They do not simulate the cuts of the steps above
//! them, so one card can appear in more than one step.
//!
//! Pure: no game state, no I/O, no randomness. Same estimate → identical explanation.
//!
//! Bracket policy is **not** part of the Comprehensive Rules — it is WotC's
//! Commander Format Panel guidance. No Comprehensive Rules annotations apply.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::database::{CardDatabase, ComboOutcome};
use crate::game::bracket_estimate::{
    cards_until_cleared, combo_declaration_rule, AxisReading, BracketAxis, BracketCheck,
    BracketCheckOutcome, BracketEstimate, ComboDeclaration, ComboFloorTrigger,
    CommanderBracketTier, BASE_FLOOR,
};
use crate::types::ability::Comparator;

/// A card the explanation names, tagged with where it sits in the deck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BracketCard {
    pub name: String,
    pub role: BracketCardRole,
}

/// Where a named card sits. The commander is fixed: a cut never names one while the
/// deck's other cards can clear the reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BracketCardRole {
    Commander,
    Deck,
}

/// A matched two-card combo, with its pieces tagged by role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplainedCombo {
    pub pieces: [BracketCard; 2],
    pub outcomes: BTreeSet<ComboOutcome>,
    pub assemble_cost: u16,
}

/// What raised a reason's floor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BracketReasonSource {
    /// A `FLOOR_RULES` row fired on a card count.
    CardRule {
        axis: BracketAxis,
        comparator: Comparator,
        threshold: u8,
        observed: u8,
        /// The tier just below `floor`, which this rule rules out.
        tier_below: CommanderBracketTier,
        /// The most `tier_below` permits: `observed - cards_until_cleared`.
        allowed_below: u8,
        /// Cards counted by name, in deck order (commander first).
        cards: Vec<BracketCard>,
        /// Combo pairs counted on this axis (`combo_contributions`).
        combos: Vec<ExplainedCombo>,
    },
    /// A `COMBO_FLOOR_RULES` row fired. Only combos not shown at a higher tier.
    ComboFloor {
        trigger: ComboFloorTrigger,
        combos: Vec<ExplainedCombo>,
    },
    /// The owner's own answer to the two-card-combo barometer.
    OwnerComboAnswer { declaration: ComboDeclaration },
}

/// One thing holding the deck at `floor`: its source, the official line it rests on,
/// and the cuts that clear it. Wire-visible, so every provenance field is owned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BracketReason {
    pub floor: CommanderBracketTier,
    pub source: BracketReasonSource,
    /// The official sentence the rule encodes, quoted verbatim.
    pub official_line: String,
    pub source_document: String,
    /// ISO-8601.
    pub source_published: String,
    pub source_url: String,
    /// Taken together, sufficient to clear this reason.
    pub cuts: Vec<BracketCut>,
}

/// One change that, with the reason's other cuts, clears it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BracketCut {
    /// Remove `count` of `from`. `from` holds deck cards only; `count == from.len()` means all.
    RemoveCards { count: u8, from: Vec<BracketCard> },
    /// Break `combo` by removing any one of `remove_one_of` (deck cards only, never empty).
    BreakCombo {
        combo: ExplainedCombo,
        remove_one_of: Vec<BracketCard>,
    },
    /// No deck-card cut is enough: `count` of the commanders in `of` must change.
    ReplaceCommander { count: u8, of: Vec<BracketCard> },
    /// The reason is the owner's combo answer.
    ChangeComboAnswer,
}

/// One rung of the ladder down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BracketLadderStep {
    /// The tier the deck lands on once this step and every step above it are done.
    pub lands_on: CommanderBracketTier,
    pub reasons: Vec<BracketReason>,
}

/// Why the deck sits at its estimated tier, and the ladder of steps down.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BracketExplanation {
    /// Above the base floor: the reasons whose floor is the estimated tier. At the base
    /// floor: the base-floor rules that fired (they rule out declaring Exhibition), possibly none.
    pub why: Vec<BracketReason>,
    /// Highest first; empty exactly when the estimate is at the base floor.
    pub steps: Vec<BracketLadderStep>,
}

/// Tags card names with their [`BracketCardRole`] against the deck's commanders.
pub(crate) struct CardRoles<'a> {
    /// The commanders' `CardDatabase::lookup_key`s (`ResolvedComboDeck::commanders`).
    commanders: &'a BTreeSet<String>,
    db: &'a CardDatabase,
}

impl<'a> CardRoles<'a> {
    pub(crate) fn new(commanders: &'a BTreeSet<String>, db: &'a CardDatabase) -> Self {
        Self { commanders, db }
    }

    /// The card `name` names: a commander when its lookup key is a commander's.
    pub(crate) fn card(&self, name: &str) -> BracketCard {
        let role = if self.commanders.contains(&self.db.lookup_key(name)) {
            BracketCardRole::Commander
        } else {
            BracketCardRole::Deck
        };
        BracketCard {
            name: name.to_owned(),
            role,
        }
    }
}

/// Explains `estimate`. `card_readings` are its axis readings before combo pairs were
/// added, so a reason lists the cards counted by name apart from the pairs.
///
/// Every fired card rule and the owner's combo answer become reasons, grouped by floor;
/// within a tier, card rules come first in `checks` order, then the owner's answer.
/// The groups above the base floor form the ladder, highest first; the top group is
/// `why`. With no group above the base floor there are no steps, and `why` holds the
/// base-floor reasons.
pub(crate) fn explain(
    estimate: &BracketEstimate,
    card_readings: &BTreeMap<BracketAxis, AxisReading>,
    roles: &CardRoles<'_>,
) -> BracketExplanation {
    let card_rules = estimate
        .checks
        .iter()
        .filter(|check| check.outcome == BracketCheckOutcome::Fired)
        .map(|check| card_rule_reason(check, card_readings, roles));
    let owner_answer = owner_combo_answer_reason(estimate.combo_barometer.declaration);
    ladder(estimate.tier, group_by_tier(card_rules.chain(owner_answer)))
}

/// A fired card rule as a reason: the cards it counted, the most the tier below allows,
/// and the fewest removals that clear it.
fn card_rule_reason(
    check: &BracketCheck,
    card_readings: &BTreeMap<BracketAxis, AxisReading>,
    roles: &CardRoles<'_>,
) -> BracketReason {
    let need = cards_until_cleared(check.comparator, check.threshold, check.observed)
        .expect("every fired floor rule clears by removing cards");
    let tier_below = check
        .floor
        .below()
        .expect("every fired floor is at least Core, which has a tier below it");
    let cards: Vec<BracketCard> = card_readings
        .get(&check.axis)
        .map_or_else(Vec::new, |reading| {
            reading
                .contributing
                .iter()
                .map(|name| roles.card(name))
                .collect()
        });
    let cuts = card_rule_cuts(&cards, need);
    BracketReason {
        floor: check.floor,
        source: BracketReasonSource::CardRule {
            axis: check.axis,
            comparator: check.comparator,
            threshold: check.threshold,
            observed: check.observed,
            tier_below,
            allowed_below: check.observed - need,
            cards,
            combos: Vec::new(),
        },
        official_line: check.official_line.clone(),
        source_document: check.source_document.clone(),
        source_published: check.source_published.clone(),
        source_url: check.source_url.clone(),
        cuts,
    }
}

/// The cuts that remove `need` of a card rule's `cards`: deck cards first, and
/// commanders only for what the deck cards cannot cover.
fn card_rule_cuts(cards: &[BracketCard], need: u8) -> Vec<BracketCut> {
    let mut deck_cards = Vec::new();
    let mut commanders = Vec::new();
    for card in cards {
        match card.role {
            BracketCardRole::Deck => deck_cards.push(card.clone()),
            BracketCardRole::Commander => commanders.push(card.clone()),
        }
    }

    let mut cuts = Vec::new();
    let removed = need.min(u8::try_from(deck_cards.len()).unwrap_or(u8::MAX));
    if removed > 0 {
        cuts.push(BracketCut::RemoveCards {
            count: removed,
            from: deck_cards,
        });
    }
    let remaining = need - removed;
    if remaining > 0 {
        cuts.push(BracketCut::ReplaceCommander {
            count: remaining,
            of: commanders,
        });
    }
    cuts
}

/// The owner's answer to the combo barometer, cited against the published combo line
/// it falls under. `None` when the owner claimed no combo.
fn owner_combo_answer_reason(declaration: ComboDeclaration) -> Option<BracketReason> {
    combo_declaration_rule(declaration).map(|rule| BracketReason {
        floor: rule.floor,
        source: BracketReasonSource::OwnerComboAnswer { declaration },
        official_line: rule.official_line.to_owned(),
        source_document: rule.source.document.to_owned(),
        source_published: rule.source.published.to_owned(),
        source_url: rule.source.url.to_owned(),
        cuts: vec![BracketCut::ChangeComboAnswer],
    })
}

/// The reasons whose floor is `tier`, in the order they were produced.
struct TierReasons {
    tier: CommanderBracketTier,
    reasons: Vec<BracketReason>,
}

/// Groups reasons by floor, highest tier first, keeping each tier's reasons in the
/// order given. Tiers order through `as_u8()`: `CommanderBracketTier` derives no `Ord`.
fn group_by_tier(reasons: impl IntoIterator<Item = BracketReason>) -> Vec<TierReasons> {
    let mut tiers: BTreeMap<Reverse<u8>, TierReasons> = BTreeMap::new();
    for reason in reasons {
        tiers
            .entry(Reverse(reason.floor.as_u8()))
            .or_insert_with(|| TierReasons {
                tier: reason.floor,
                reasons: Vec::new(),
            })
            .reasons
            .push(reason);
    }
    tiers.into_values().collect()
}

/// Builds `why` and the ladder from tier groups ordered highest first. Each step lands
/// on the next group's tier, and the last lands on the base floor.
fn ladder(estimated: CommanderBracketTier, tiers: Vec<TierReasons>) -> BracketExplanation {
    let (above, rest): (Vec<TierReasons>, Vec<TierReasons>) = tiers
        .into_iter()
        .partition(|group| group.tier.as_u8() > BASE_FLOOR.as_u8());

    let why = match above.first() {
        Some(top) => {
            debug_assert_eq!(
                top.tier, estimated,
                "the highest reason's floor is the estimated tier"
            );
            top.reasons.clone()
        }
        None => rest
            .into_iter()
            .filter(|group| group.tier == BASE_FLOOR)
            .flat_map(|group| group.reasons)
            .collect(),
    };

    let landings: Vec<CommanderBracketTier> = above
        .iter()
        .skip(1)
        .map(|group| group.tier)
        .chain(std::iter::once(BASE_FLOOR))
        .collect();
    let steps = above
        .into_iter()
        .zip(landings)
        .map(|(group, lands_on)| BracketLadderStep {
            lands_on,
            reasons: group.reasons,
        })
        .collect();

    BracketExplanation { why, steps }
}
