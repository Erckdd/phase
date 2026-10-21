//! The bracket explanation: the reasons at the estimated tier ("Why") and the
//! ladder of steps down ("Bring it down"), each reason carrying typed cuts.

use engine::database::{BracketCardClass, BracketLists, CardDatabase, ComboTable};
use engine::game::{
    estimate_bracket, BracketAxis, BracketCard, BracketCardRole, BracketCut, BracketEstimate,
    BracketExplanation, BracketLadderStep, BracketReasonSource, ComboDeclaration, ComboWindow,
    CommanderBracketTier, PlayerDeckList,
};

fn db(game_changers: &[&str], mass_land_denial: &[&str], extra_turns: &[&str]) -> CardDatabase {
    CardDatabase::default().with_bracket_lists(BracketLists::from_pairs(
        "explain-test",
        &[
            (BracketCardClass::GameChangers, game_changers),
            (BracketCardClass::MassLandSweepers, mass_land_denial),
            (BracketCardClass::ExtraTurns, extra_turns),
        ],
    ))
}

fn deck(commander: &[&str], main: &[&str]) -> PlayerDeckList {
    PlayerDeckList {
        commander: commander.iter().map(|name| (*name).to_owned()).collect(),
        main_deck: main.iter().map(|name| (*name).to_owned()).collect(),
        ..Default::default()
    }
}

fn card(name: &str) -> BracketCard {
    BracketCard {
        name: name.to_owned(),
        role: BracketCardRole::Deck,
    }
}

fn commander(name: &str) -> BracketCard {
    BracketCard {
        name: name.to_owned(),
        role: BracketCardRole::Commander,
    }
}

fn estimate(deck: &PlayerDeckList, db: &CardDatabase) -> BracketEstimate {
    estimate_bracket(deck, db, &ComboTable::default()).unwrap()
}

const TERGRID: &str = "Tergrid, God of Fright";

fn tergrid_estimate() -> BracketEstimate {
    estimate(
        &deck(&[TERGRID], &["Rhystic Study", "Cyclonic Rift"]),
        &db(&[TERGRID, "Rhystic Study", "Cyclonic Rift"], &[], &[]),
    )
}

#[test]
fn five_game_changers_step_down_two_then_all() {
    let five = ["A", "B", "C", "D", "E"];
    let estimate = estimate(&deck(&["Cmdr"], &five), &db(&five, &[], &[]));
    let e = &estimate.explanation;
    let all_five: Vec<BracketCard> = five.iter().map(|name| card(name)).collect();

    assert_eq!(estimate.tier, CommanderBracketTier::Optimized);
    assert_eq!(e.why.len(), 1, "only the >= 4 rule holds the deck at B4");
    assert!(matches!(
        &e.why[0].source,
        BracketReasonSource::CardRule {
            axis: BracketAxis::GameChangers,
            threshold: 4,
            observed: 5,
            allowed_below: 3,
            tier_below: CommanderBracketTier::Upgraded,
            ..
        }
    ));
    assert_eq!(
        e.steps.iter().map(|step| step.lands_on).collect::<Vec<_>>(),
        [CommanderBracketTier::Upgraded, CommanderBracketTier::Core]
    );
    assert_eq!(
        e.steps[0].reasons[0].cuts,
        [BracketCut::RemoveCards {
            count: 2,
            from: all_five.clone(),
        }]
    );
    assert_eq!(
        e.steps[1].reasons[0].cuts,
        [BracketCut::RemoveCards {
            count: 5,
            from: all_five,
        }]
    );
    assert_eq!(e.steps[0].reasons, e.why);
}

#[test]
fn a_clean_deck_has_no_reasons_and_no_steps() {
    let estimate = estimate(&deck(&["Cmdr"], &["Forest"]), &db(&[], &[], &[]));

    assert_eq!(estimate.tier, CommanderBracketTier::Core);
    assert_eq!(estimate.explanation, BracketExplanation::default());
}

#[test]
fn an_extra_turn_card_is_a_why_reason_at_the_base_floor_but_never_a_step() {
    let estimate = estimate(
        &deck(&["Cmdr"], &["Time Warp"]),
        &db(&[], &[], &["Time Warp"]),
    );
    let e = &estimate.explanation;

    assert_eq!(estimate.tier, CommanderBracketTier::Core);
    assert_eq!(e.why.len(), 1);
    assert_eq!(e.why[0].floor, CommanderBracketTier::Core);
    let BracketReasonSource::CardRule {
        axis,
        tier_below,
        allowed_below,
        cards,
        combos,
        ..
    } = &e.why[0].source
    else {
        panic!("expected a card rule, got {:?}", e.why[0].source);
    };
    assert_eq!(*axis, BracketAxis::ExtraTurns);
    assert_eq!(*tier_below, CommanderBracketTier::Exhibition);
    assert_eq!(*allowed_below, 0);
    assert_eq!(*cards, [card("Time Warp")]);
    assert!(combos.is_empty());
    assert!(e.steps.is_empty(), "the ladder ends at the base floor");
}

#[test]
fn the_owners_early_combo_answer_is_its_own_reason_with_no_table() {
    let declaration = ComboDeclaration::Intended {
        window: Some(ComboWindow::EarlyGame),
    };
    let combo_deck = PlayerDeckList {
        combo_declaration: declaration,
        ..deck(&["Cmdr"], &["Forest"])
    };
    let estimate = estimate(&combo_deck, &db(&[], &[], &[]));
    let e = &estimate.explanation;

    assert_eq!(estimate.tier, CommanderBracketTier::Optimized);
    assert_eq!(e.why.len(), 1);
    assert_eq!(e.why[0].floor, CommanderBracketTier::Optimized);
    assert_eq!(
        e.why[0].source,
        BracketReasonSource::OwnerComboAnswer { declaration }
    );
    assert_eq!(e.why[0].cuts, [BracketCut::ChangeComboAnswer]);
    assert!(e.why[0]
        .official_line
        .starts_with("No intentional early-game two-card infinite combos."));
    assert_eq!(
        e.steps,
        [BracketLadderStep {
            lands_on: CommanderBracketTier::Core,
            reasons: e.why.clone(),
        }]
    );
}

#[test]
fn a_game_changer_commander_is_tagged_and_never_cut() {
    let estimate = tergrid_estimate();
    let e = &estimate.explanation;

    assert_eq!(estimate.tier, CommanderBracketTier::Upgraded);
    let BracketReasonSource::CardRule { cards, .. } = &e.why[0].source else {
        panic!("expected a card rule, got {:?}", e.why[0].source);
    };
    assert_eq!(
        *cards,
        [
            commander(TERGRID),
            card("Rhystic Study"),
            card("Cyclonic Rift"),
        ]
    );
    assert_eq!(
        e.why[0].cuts,
        [
            BracketCut::RemoveCards {
                count: 2,
                from: vec![card("Rhystic Study"), card("Cyclonic Rift")],
            },
            BracketCut::ReplaceCommander {
                count: 1,
                of: vec![commander(TERGRID)],
            },
        ]
    );
}

#[test]
fn under_the_four_rule_a_commander_game_changer_is_left_out_of_the_cut() {
    let four = ["A", "B", "C", "D"];
    let estimate = estimate(
        &deck(&[TERGRID], &four),
        &db(&[TERGRID, "A", "B", "C", "D"], &[], &[]),
    );
    let e = &estimate.explanation;
    let all_four: Vec<BracketCard> = four.iter().map(|name| card(name)).collect();

    assert_eq!(estimate.tier, CommanderBracketTier::Optimized);
    assert_eq!(
        e.steps[0].reasons[0].cuts,
        [BracketCut::RemoveCards {
            count: 2,
            from: all_four.clone(),
        }]
    );
    assert_eq!(
        e.steps[1].reasons[0].cuts,
        [
            BracketCut::RemoveCards {
                count: 4,
                from: all_four,
            },
            BracketCut::ReplaceCommander {
                count: 1,
                of: vec![commander(TERGRID)],
            },
        ]
    );
}

#[test]
fn partner_game_changer_commanders_need_both_replaced_under_the_one_rule() {
    let estimate = estimate(&deck(&["P1", "P2"], &[]), &db(&["P1", "P2"], &[], &[]));
    let e = &estimate.explanation;

    assert_eq!(estimate.tier, CommanderBracketTier::Upgraded);
    assert_eq!(
        e.why[0].cuts,
        [BracketCut::ReplaceCommander {
            count: 2,
            of: vec![commander("P1"), commander("P2")],
        }]
    );
}

#[test]
fn explanation_wire_shape_is_snake_case_and_kind_tagged() {
    let value = serde_json::to_value(tergrid_estimate()).unwrap();
    let why = &value["explanation"]["why"][0];

    assert_eq!(why["source"]["kind"], "card_rule");
    assert_eq!(why["source"]["tier_below"], "core");
    assert_eq!(
        why["cuts"][0],
        serde_json::json!({
            "kind": "remove_cards",
            "count": 2,
            "from": [
                { "name": "Rhystic Study", "role": "deck" },
                { "name": "Cyclonic Rift", "role": "deck" },
            ],
        })
    );
    assert_eq!(why["cuts"][1]["kind"], "replace_commander");
    assert_eq!(why["cuts"][1]["of"][0]["role"], "commander");
}

#[test]
fn an_estimate_without_an_explanation_still_deserializes() {
    let estimate = tergrid_estimate();
    let mut value = serde_json::to_value(&estimate).unwrap();

    assert_eq!(
        serde_json::from_value::<BracketEstimate>(value.clone()).unwrap(),
        estimate,
        "the explanation round-trips"
    );
    value.as_object_mut().unwrap().remove("explanation");
    assert_eq!(
        serde_json::from_value::<BracketEstimate>(value)
            .unwrap()
            .explanation,
        BracketExplanation::default()
    );
}
