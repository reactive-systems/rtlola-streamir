use streamir_lib::formatter::files::{FilesFormatter, Requirement};

use crate::{
    constructs::{RequirementKey, WindowsKey},
    RustFormatter,
};

use super::AggregationTrait;

pub(crate) struct ConditionalInstanceAggregationFunction;

impl Requirement<RustFormatter> for ConditionalInstanceAggregationFunction {
    fn key(&self) -> <RustFormatter as FilesFormatter>::Key {
        RequirementKey::Windows(WindowsKey::ConditionalInstanceAggregation)
    }

    fn file(&self, formatter: &RustFormatter) -> std::path::PathBuf {
        formatter.windows_file()
    }

    fn format(self, formatter: &RustFormatter) -> String {
        formatter.add_requirement(AggregationTrait);
        r#"impl<Parameter: Eq + std::hash::Hash + Clone, StreamType: Clone, const STREAM_SIZE: usize>
    InstanceStreamBuffer<Parameter, StreamType, STREAM_SIZE>
{
    pub(crate) fn cond_aggregate_instances<'a, Aggregation: WindowAggregation>(
        &'a self,
        fresh: bool,
        filter: impl Fn(Parameter) -> bool,
    ) -> Aggregation::ResultType
    where
        StreamType: Into<Aggregation::ValueType>,
    {
        Aggregation::lower(
            self.stream_buffer
                .iter()
                .filter(|(_, buffer)| !fresh || buffer.is_fresh())
                .filter(|(param, _)| filter((*param).clone()))
                .filter_map(|(_, buffer)| buffer.get(0).unwrap())
                .fold(Aggregation::initial_value(Duration::new(0, 0)), |a, value| {
                    Aggregation::aggregate(
                        a,
                        Aggregation::map(value.clone().into(), Duration::new(0, 0)),
                    )
                }),
        )
    }
}"#.into()
    }
}
