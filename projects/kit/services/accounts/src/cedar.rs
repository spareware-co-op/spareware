//! Cedar, as the answer to [`kit_ops::Decider`]. It knows no entity and no action: a program brings
//! its own rules and schema, and asks in Cedar's own JSON. Every question is checked against the
//! schema, so a program whose code and rules have drifted apart gets an error, not a wrong answer.

use cedar_policy::{
    Authorizer, Context, Decision, Entities, EntityUid, PolicySet, Request, Schema, ValidationMode,
    Validator,
};
use kit_ops::{Decider, Question};
use std::str::FromStr;

pub struct Cedar {
    policies: PolicySet,
    schema: Schema,
    authorizer: Authorizer,
}

fn plain(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn entity_uid((kind, id): &(String, String)) -> Result<EntityUid, String> {
    // The ID's alphabet holds nothing that needs quoting.
    EntityUid::from_str(&format!("{kind}::\"{id}\"")).map_err(plain)
}

impl Cedar {
    /// Loads the rules, and refuses to start if they don't fit the schema.
    pub fn new(policy: &str, schema: &str) -> Result<Self, String> {
        let (schema, _warnings) = Schema::from_cedarschema_str(schema).map_err(plain)?;
        let policies = PolicySet::from_str(policy).map_err(plain)?;
        let check = Validator::new(schema.clone()).validate(&policies, ValidationMode::Strict);
        if !check.validation_passed() {
            return Err(format!("the access rules don't fit the schema: {check}"));
        }
        Ok(Self {
            policies,
            schema,
            authorizer: Authorizer::new(),
        })
    }
}

impl Decider for Cedar {
    fn decide(&self, question: &Question) -> Result<bool, String> {
        let action = EntityUid::from_str(&question.action).map_err(plain)?;
        let context =
            Context::from_json_value(question.context.clone(), Some((&self.schema, &action)))
                .map_err(plain)?;
        let request = Request::new(
            entity_uid(&question.principal)?,
            action,
            entity_uid(&question.resource)?,
            context,
            Some(&self.schema),
        )
        .map_err(plain)?;
        let entities = Entities::from_json_value(question.entities.clone(), Some(&self.schema))
            .map_err(plain)?;
        let answer = self
            .authorizer
            .is_authorized(&request, &self.policies, &entities);
        Ok(answer.decision() == Decision::Allow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SCHEMA: &str = r#"
        namespace App {
          entity Thing;
          entity Person in [Thing];
          action "open" appliesTo { principal: Person, resource: Thing };
        }
    "#;
    const POLICY: &str = r#"permit (principal, action == App::Action::"open", resource) when { principal in resource };"#;

    fn question(parents: serde_json::Value) -> Question {
        Question {
            principal: ("App::Person".to_owned(), "a".to_owned()),
            action: r#"App::Action::"open""#.to_owned(),
            resource: ("App::Thing".to_owned(), "t".to_owned()),
            context: json!({}),
            entities: json!([
                { "uid": { "type": "App::Person", "id": "a" }, "attrs": {}, "parents": parents },
                { "uid": { "type": "App::Thing", "id": "t" }, "attrs": {}, "parents": [] },
            ]),
        }
    }

    #[test]
    fn the_answer_comes_from_the_rules_and_the_entities() {
        let cedar = Cedar::new(POLICY, SCHEMA).unwrap();
        let member = question(json!([{ "type": "App::Thing", "id": "t" }]));
        assert_eq!(cedar.decide(&member), Ok(true));
        assert_eq!(cedar.decide(&question(json!([]))), Ok(false));
    }

    #[test]
    fn rules_that_dont_fit_the_schema_stop_the_server() {
        let wrong = r#"permit (principal, action == App::Action::"close", resource);"#;
        assert!(Cedar::new(wrong, SCHEMA).is_err());
    }

    #[test]
    fn a_question_the_schema_doesnt_know_is_an_error() {
        let cedar = Cedar::new(POLICY, SCHEMA).unwrap();
        let mut odd = question(json!([]));
        odd.action = r#"App::Action::"close""#.to_owned();
        assert!(cedar.decide(&odd).is_err());
    }
}
