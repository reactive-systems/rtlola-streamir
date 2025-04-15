use std::time::Duration;

use streamir_lib::{
    formatter::names::GetStreamName,
    function_names,
    ir::{StreamReference, WindowReference},
};

use crate::{RustFormatter, RustType};

impl GetStreamName for RustFormatter {
    fn stream_name(&self, sr: StreamReference) -> String {
        self.sr2name[&sr].clone()
    }

    fn window_name(&self, wref: WindowReference) -> String {
        match wref {
            WindowReference::Sliding(i) => format!("sliding{i}"),
            WindowReference::Discrete(i) => format!("discrete{i}"),
            WindowReference::Instance(i) => format!("instance{i}"),
        }
    }
}

function_names! {RustFormatter,
    /// The type name of topmost monitor struct
    monitor_struct_name(): "Monitor",

    /// The function name of the cycle function containing the statement
    cycle_function_name(): "cycle",

    /// The function name of the input statement function
    input_statement_function_name(stream): "eval_{stream}",

    /// The function name of the eval statement function
    eval_statement_function_name(stream, num): "eval_{stream}_{num}",

    /// The function name of the syn access function to a stream
    sync_access_function_name(stream): "{stream}",

    /// The function name of the offset access function to a stream
    offset_access_function_name(stream): "{stream}_offset",

    /// The function name of the hold access function to a stream
    hold_access_function_name(stream): "{stream}_hold",

    /// The function name of the is fresh access function to a stream
    is_fresh_access_function_name(stream): "{stream}_is_fresh",

    /// The function name of the get access function to a stream
    get_access_function_name(stream): "{stream}_get",

    /// The function name of the function evaluating an expression
    expr_function_name(num): "expr_{num}",

    /// The name used in the cycle function for the num'th parameter
    cycle_parameter_name(num): "p{num}",

    /// The name used in the cycle function for the parameter of an instance aggregation lambda
    instance_aggregation_parameter_name(num): "ia{num}",

    /// The name of the error
    error_name(): "MonitorError",

    /// The name of the argument holding the start time of the monitor
    start_time_argument_name(): "start_time",
    /// The name of the argument holding the time of an event
    time_argument_name(): "time",

    /// The name of the function to clear all activations of streams at the end of the cycle
    clear_activations_function_name(): "clear_activations",

    /// The name of the function to clear the activation of a buffer at the end of the cycle
    clear_activation_function_name(): "clear_activation",

    /// The method of the instance buffer returning all fresh instances with their value
    fresh_instances_function(): "fresh_instances",

    /// The name of the deadline enum
    deadline_enum_name(): "Deadline",

    /// The name of the state struct that is contained in the queue
    state_struct_name(): "State",

    /// The name of the function to construct a new state with the deadline after the period
    state_new_after_function_name(): "new_after",

    /// The name of the stream reference enum
    stream_reference_name(): "StreamReference",

    /// The name of the queue struct
    queue_struct_name(): "Queue",

    /// The name of the function to pop a state of the queue
    queue_pop_function_name(): "pop",

    /// The name of the function to push a state to the queue
    queue_push_function_name(): "push",

    /// The name of the function to push a state to the queue
    queue_collect_and_add_function_name(): "collect_and_add",

    queue_remove_function_name(): "remove",

    /// The name of the function to get the next event from the queue
    queue_next_function_name(): "next",

    /// Constructs a new periodic internal event
    internal_event_from_deadline_function_name(): "new_periodic_event",

    /// The name of the windows memory struct
    windows_memory_struct_name(): "WindowsMemory",

    /// The name of the function printing the csv header for the verdicts
    verdict_header_function(): "header",

    spawned_argument_name(): "spawned",

    closed_argument_name(): "closed",

    schedule_clear_function(): "clear"
}

impl RustFormatter {
    pub(crate) fn time_argument(&self) -> (String, RustType) {
        (self.time_argument_name(), RustType::Duration)
    }

    pub(crate) fn start_time_argument(&self) -> (String, RustType) {
        (self.start_time_argument_name(), RustType::Duration)
    }

    pub(crate) fn format_duration_name(&self, d: Duration) -> String {
        format!("{}", d.as_millis())
    }

    pub(crate) fn stream_reference_variant(&self, sr: StreamReference) -> String {
        let name = self.stream_name(sr);
        let mut chars = name.chars();
        format!(
            "{}{}",
            chars.next().unwrap().to_ascii_uppercase(),
            chars.collect::<String>()
        )
    }

    pub(crate) fn dynamic_deadline_event_name(&self, dl: Duration) -> String {
        format!("dynamic{}", self.format_duration_name(dl))
    }

    pub(crate) fn static_deadline_event_name(&self, dl: Duration) -> String {
        format!("static{}", self.format_duration_name(dl))
    }
}
