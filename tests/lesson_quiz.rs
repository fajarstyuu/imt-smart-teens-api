//! Behavior checks for the lesson-linked quiz domain contract.
use imt_smart_teens_backend::modules::{
    assessment::lesson_quiz::{LessonQuizDraft, LessonQuizOption},
    learning::admin::application::{LessonDraft, SectionDraft},
};
use uuid::Uuid;

fn valid_quiz() -> LessonQuizDraft {
    LessonQuizDraft {
        text: "Manakah jawaban yang benar?".into(),
        options: vec![
            LessonQuizOption {
                id: Uuid::from_u128(1),
                text: "A".into(),
            },
            LessonQuizOption {
                id: Uuid::from_u128(2),
                text: "B".into(),
            },
        ],
        correct_option_id: Some(Uuid::from_u128(1)),
    }
}

#[test]
fn lesson_cannot_publish_without_its_own_quiz() {
    let lesson = LessonDraft {
        title: "Materi".into(),
        description: "Pengantar".into(),
        sections: vec![SectionDraft {
            id: Uuid::from_u128(4),
            title: "Bagian".into(),
            body: "Isi materi".into(),
            required: true,
        }],
        quiz: None,
    };

    assert!(lesson.validate(true).is_err());
    assert!(lesson.validate(false).is_ok());
}

#[test]
fn publishable_lesson_quiz_requires_two_ordered_options_and_one_key() {
    let quiz = valid_quiz();
    assert!(quiz.validate(true).is_ok());

    let mut missing_key = valid_quiz();
    missing_key.correct_option_id = None;
    assert!(missing_key.validate(true).is_err());

    let mut three_options = valid_quiz();
    three_options.options.push(LessonQuizOption {
        id: Uuid::from_u128(3),
        text: "C".into(),
    });
    assert!(three_options.validate(true).is_err());
}

#[test]
fn incomplete_lesson_quiz_is_allowed_only_while_drafting() {
    let mut quiz = valid_quiz();
    quiz.correct_option_id = None;
    assert!(quiz.validate(false).is_ok());
    assert!(quiz.validate(true).is_err());
}
