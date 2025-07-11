use std::collections::BTreeMap;
use vrl::prelude::*;

use crate::vrl_util::is_case_sensitive;
use crate::{
    vrl_util::{self, add_index, evaluate_condition},
    Case, Condition, IndexHandle, TableRegistry, TableSearch,
};

fn find_enrichment_table_records(
    select: Option<Value>,
    enrichment_tables: &TableSearch,
    table: &str,
    case_sensitive: Case,
    wildcard: Option<Value>,
    condition: &[Condition],
    index: Option<IndexHandle>,
) -> Resolved {
    let select = select
        .map(|select| match select {
            Value::Array(arr) => arr
                .iter()
                .map(|value| Ok(value.try_bytes_utf8_lossy()?.to_string()))
                .collect::<std::result::Result<Vec<_>, _>>(),
            value => Err(ValueError::Expected {
                got: value.kind(),
                expected: Kind::array(Collection::any()),
            }),
        })
        .transpose()?;

    let data = enrichment_tables
        .find_table_rows(
            table,
            case_sensitive,
            condition,
            select.as_ref().map(|select| select.as_ref()),
            wildcard.as_ref(),
            index,
        )?
        .into_iter()
        .map(Value::Object)
        .collect();
    Ok(Value::Array(data))
}

#[derive(Clone, Copy, Debug)]
pub struct FindEnrichmentTableRecords;
impl Function for FindEnrichmentTableRecords {
    fn identifier(&self) -> &'static str {
        "find_enrichment_table_records"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "table",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "condition",
                kind: kind::OBJECT,
                required: true,
            },
            Parameter {
                keyword: "select",
                kind: kind::ARRAY,
                required: false,
            },
            Parameter {
                keyword: "case_sensitive",
                kind: kind::BOOLEAN,
                required: false,
            },
            Parameter {
                keyword: "wildcard",
                kind: kind::BYTES,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "find records",
            source: r#"find_enrichment_table_records!("test", {"surname": "Smith"})"#,
            result: Ok(
                indoc! { r#"[{"id": 1, "firstname": "Bob", "surname": "Smith"},
                             {"id": 2, "firstname": "Fred", "surname": "Smith"}]"#,
                },
            ),
        }]
    }

    fn compile(
        &self,
        state: &TypeState,
        ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let registry = ctx
            .get_external_context_mut::<TableRegistry>()
            .ok_or(Box::new(vrl_util::Error::TablesNotLoaded) as Box<dyn DiagnosticMessage>)?;

        let tables = registry
            .table_ids()
            .into_iter()
            .map(Value::from)
            .collect::<Vec<_>>();

        let table = arguments
            .required_enum("table", &tables, state)?
            .try_bytes_utf8_lossy()
            .expect("table is not valid utf8")
            .into_owned();
        let condition = arguments.required_object("condition")?;

        let select = arguments.optional("select");

        let case_sensitive = is_case_sensitive(&arguments, state)?;
        let wildcard = arguments.optional("wildcard");
        let index = Some(
            add_index(registry, &table, case_sensitive, &condition)
                .map_err(|err| Box::new(err) as Box<_>)?,
        );

        Ok(FindEnrichmentTableRecordsFn {
            table,
            condition,
            index,
            select,
            case_sensitive,
            wildcard,
            enrichment_tables: registry.as_readonly(),
        }
        .as_expr())
    }
}

#[derive(Debug, Clone)]
pub struct FindEnrichmentTableRecordsFn {
    table: String,
    condition: BTreeMap<KeyString, expression::Expr>,
    index: Option<IndexHandle>,
    select: Option<Box<dyn Expression>>,
    case_sensitive: Case,
    wildcard: Option<Box<dyn Expression>>,
    enrichment_tables: TableSearch,
}

impl FunctionExpression for FindEnrichmentTableRecordsFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let condition = self
            .condition
            .iter()
            .map(|(key, value)| {
                let value = value.resolve(ctx)?;
                evaluate_condition(key, value)
            })
            .collect::<ExpressionResult<Vec<Condition>>>()?;

        let select = self
            .select
            .as_ref()
            .map(|array| array.resolve(ctx))
            .transpose()?;

        let table = &self.table;
        let case_sensitive = self.case_sensitive;
        let wildcard = self
            .wildcard
            .as_ref()
            .map(|array| array.resolve(ctx))
            .transpose()?;
        let index = self.index;
        let enrichment_tables = &self.enrichment_tables;

        find_enrichment_table_records(
            select,
            enrichment_tables,
            table,
            case_sensitive,
            wildcard,
            &condition,
            index,
        )
    }

    fn type_def(&self, _: &TypeState) -> TypeDef {
        TypeDef::array(Collection::from_unknown(Kind::object(Collection::any()))).fallible()
    }
}

#[cfg(test)]
mod tests {
    use vrl::compiler::state::RuntimeState;
    use vrl::compiler::TargetValue;
    use vrl::compiler::TimeZone;
    use vrl::value;
    use vrl::value::Secrets;

    use super::*;
    use crate::test_util::get_table_registry;

    #[test]
    fn find_table_row() {
        let registry = get_table_registry();
        let func = FindEnrichmentTableRecordsFn {
            table: "dummy1".to_string(),
            condition: BTreeMap::from([(
                "field".into(),
                expression::Literal::from("value").into(),
            )]),
            index: Some(IndexHandle(999)),
            select: None,
            case_sensitive: Case::Sensitive,
            wildcard: None,
            enrichment_tables: registry.as_readonly(),
        };

        let tz = TimeZone::default();
        let object: Value = ObjectMap::new().into();
        let mut target = TargetValue {
            value: object,
            metadata: value!({}),
            secrets: Secrets::new(),
        };
        let mut runtime_state = RuntimeState::default();
        let mut ctx = Context::new(&mut target, &mut runtime_state, &tz);

        registry.finish_load();

        let got = func.resolve(&mut ctx);

        assert_eq!(Ok(value![vec![value!({ "field": "result" })]]), got);
    }
}
