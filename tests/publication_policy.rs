//! Application admission accepts typed published facts without SQL, JSON or HTTP construction.
use imt_smart_teens_backend::{
    modules::assessment::{
        admin::application::{
            BankKind, OptionDraft, PublishedBank, PublishedQuestion, QuestionDraft, QuestionType,
        },
        domain::MAX_BANK_SCORE_UNITS,
    },
    shared::{editing::ContentState, error::AppError},
};
use uuid::Uuid;

fn published() -> PublishedBank {
    let bank_id = Uuid::new_v4();
    let options = vec![
        OptionDraft {
            id: Uuid::new_v4(),
            text: "One".into(),
        },
        OptionDraft {
            id: Uuid::new_v4(),
            text: "Two".into(),
        },
    ];
    PublishedBank {
        id: bank_id,
        kind: BankKind::Paired,
        state: ContentState::Draft,
        version_id: Some(Uuid::new_v4()),
        version_bank_id: Some(bank_id),
        questions: vec![PublishedQuestion {
            bank_id,
            state: ContentState::Published,
            position: 1,
            scoring_rule: "exact_set_v1".into(),
            max_points_units: 10000,
            draft: QuestionDraft {
                text: "Published text".into(),
                question_type: QuestionType::Single,
                correct_option_ids: vec![options[0].id],
                options,
                required: true,
            },
        }],
    }
}

#[test]
fn published_bank_policy_accepts_dirty_roots_but_rejects_unavailable_identity() {
    let b = published();
    assert_eq!(
        b.admit(b.id, BankKind::Paired).unwrap(),
        b.version_id.unwrap()
    );
    assert!(matches!(
        b.admit(Uuid::new_v4(), BankKind::Paired),
        Err(AppError::Conflict("CONTENT_NOT_AVAILABLE"))
    ));
    assert!(matches!(
        b.admit(b.id, BankKind::Quiz),
        Err(AppError::Conflict("CONTENT_NOT_AVAILABLE"))
    ));
    for change in [0, 1, 2, 3] {
        let mut bad = published();
        match change {
            0 => bad.state = ContentState::Archived,
            1 => bad.version_id = None,
            2 => bad.version_bank_id = Some(Uuid::new_v4()),
            _ => bad.questions.clear(),
        }
        assert!(bad.admit(bad.id, BankKind::Paired).is_err());
    }
}

#[test]
fn published_bank_policy_rejects_malformed_question_facts_and_exact_aggregate_overflow() {
    for change in 0..12 {
        let mut b = published();
        let q = &mut b.questions[0];
        match change {
            0 => q.bank_id = Uuid::new_v4(),
            1 => q.state = ContentState::Archived,
            2 => q.scoring_rule = "unknown".into(),
            3 => q.position = 0,
            4 => q.draft.correct_option_ids = vec![Uuid::new_v4()],
            5 => q.draft.correct_option_ids.clear(),
            6 => q.draft.correct_option_ids = q.draft.options.iter().map(|o| o.id).collect(),
            7 => q.draft.required = false,
            8 => q.draft.options[1].text = "One".into(),
            9 => q.draft.options.truncate(1),
            10 => q.max_points_units = 0,
            _ => {
                let duplicate = q.clone();
                b.questions.push(duplicate);
            }
        }
        assert!(
            b.admit(b.id, BankKind::Paired).is_err(),
            "accepted malformed case {change}"
        );
    }
    let mut b = published();
    b.questions[0].max_points_units = MAX_BANK_SCORE_UNITS;
    assert!(b.admit(b.id, BankKind::Paired).is_ok());
    let mut next = b.questions[0].clone();
    next.position = 2;
    next.max_points_units = 1;
    b.questions.push(next);
    assert!(matches!(
        b.admit(b.id, BankKind::Paired),
        Err(AppError::Conflict("CONTENT_NOT_AVAILABLE"))
    ));
}
