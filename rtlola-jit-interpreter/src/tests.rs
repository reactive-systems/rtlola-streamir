use std::time::Duration;

use anyhow::Context;
use streamir_lib::{parse, ParserConfig};

use crate::{
    verdict::{TotalIncremental, Verdict},
    Inputs, Monitor,
};

fn setup(spec: &str) -> (Monitor, Monitor) {
    let config = ParserConfig::for_string(spec.to_string());
    let streamir = parse(&config).context("parsing spec").unwrap();
    (
        Monitor::build(streamir.clone(), true),
        Monitor::build(streamir, false),
    )
}

fn accept_event(
    optimized: &mut Monitor,
    unoptimized: &mut Monitor,
    ts: Duration,
    inputs: Inputs,
    expected: TotalIncremental,
    expected_timed: Option<(Duration, TotalIncremental)>,
) {
    let optimized_verdict = optimized.accept_event(inputs.clone(), ts).only_value();
    let unoptimized_verdict = unoptimized.accept_event(inputs, ts).only_value();
    assert_eq!(
        &optimized_verdict, &unoptimized_verdict,
        "mismatch between optimized and unoptimized"
    );
    let Verdict {
        timed,
        ts: event_ts,
        event,
    } = optimized_verdict;
    if let Some((expected_timed_ts, expected_timed)) = expected_timed {
        assert_eq!(timed.len(), 1);
        let (timed_ts, timed) = &timed[0];
        assert_eq!(expected_timed_ts, *timed_ts);
        assert_eq!(expected_timed.sorted_inputs(), timed.sorted_inputs());
        assert_eq!(expected_timed.sorted_outputs(), timed.sorted_outputs());
    } else {
        assert!(timed.is_empty(), "{timed:?}");
    }
    assert_eq!(ts, event_ts);
    assert_eq!(expected.sorted_inputs(), event.sorted_inputs());
    assert_eq!(expected.sorted_outputs(), event.sorted_outputs());
}

macro_rules! build_input {
    ($inputs: expr) => {
        $inputs
            .clone()
            .0
            .into_iter()
            .enumerate()
            .flat_map(|(i, v)| v.map(|v| (i, v)))
            .collect()
    };
}

macro_rules! build_output {
    ($unparameterized: expr) => {
        $unparameterized
            .into_iter()
            .enumerate()
            .flat_map(|(i, v): (_, Option<Value>)| {
                v.map(|v| {
                    (
                        OutputReference::Unparameterized(i),
                        vec![Change::Value(Option::None, v)],
                    )
                })
            })
            .collect()
    };
    ($unparameterized: expr, $parameterized: expr) => {{
        let unparameterized =
            $unparameterized
                .into_iter()
                .enumerate()
                .flat_map(|(i, v): (_, Option<Value>)| {
                    v.map(|v| (Unparameterized(i), vec![Change::Value(Option::None, v)]))
                });
        let parameterized = $parameterized.into_iter().enumerate().flat_map(
            |(i, values): (_, Vec<(Vec<Value>, Value)>)| {
                if values.is_empty() {
                    Option::None
                } else {
                    Some((
                        Parameterized(i),
                        values
                            .into_iter()
                            .map(|(p, v)| Change::Value(Some(p), v))
                            .collect(),
                    ))
                }
            },
        );
        unparameterized.chain(parameterized).collect()
    }};
}

macro_rules! accept_event {
    ($optimized:expr, $unoptimized:expr, $ts:expr, $event:expr, $verdict:expr) => {{
        let inputs = Inputs($event);
        let expected_event_verdict = TotalIncremental {
            inputs: build_input!(inputs),
            outputs: build_output!($verdict),
        };
        accept_event(
            &mut $optimized,
            &mut $unoptimized,
            $ts,
            inputs,
            expected_event_verdict,
            Option::None,
        );
    }};
    ($optimized:expr, $unoptimized:expr, $ts:expr, $event:expr, $unparameterized_verdict:expr, $parameterized_verdict:expr) => {{
        let inputs = Inputs($event);
        let expected_event_verdict = TotalIncremental {
            inputs: build_input!(inputs),
            outputs: build_output!($unparameterized_verdict, $parameterized_verdict),
        };
        accept_event(
            &mut $optimized,
            &mut $unoptimized,
            $ts,
            inputs,
            expected_event_verdict,
            Option::None,
        );
    }};
    ($optimized:expr, $unoptimized:expr, $ts:expr, $event:expr, $verdict:expr, $timed_ts:expr, $timed:expr) => {{
        let inputs = Inputs($event);
        let expected_event_verdict = TotalIncremental {
            inputs: build_input!(inputs),
            outputs: build_output!($verdict),
        };
        let expected_timed = TotalIncremental {
            inputs: Vec::new(),
            outputs: build_output!($timed),
        };
        accept_event(
            &mut $optimized,
            &mut $unoptimized,
            $ts,
            inputs,
            expected_event_verdict,
            Option::Some(($timed_ts, expected_timed)),
        );
    }};
    ($optimized:expr, $unoptimized:expr, $ts:expr, $event:expr, $unparameterized_verdict:expr, $parameterized_verdict:expr, $timed_ts:expr, $timed_unparameterized_verdict:expr, $timed_parameterized_verdict:expr) => {{
        let inputs = Inputs($event);
        let expected_event_verdict = TotalIncremental {
            inputs: build_input!(inputs),
            outputs: build_output!($unparameterized_verdict, $parameterized_verdict),
        };
        let expected_timed_verdict = TotalIncremental {
            inputs: Vec::new(),
            outputs: build_output!($timed_unparameterized_verdict, $timed_parameterized_verdict),
        };
        accept_event(
            &mut $optimized,
            &mut $unoptimized,
            $ts,
            inputs,
            expected_event_verdict,
            Option::Some(($timed_ts, expected_timed_verdict)),
        );
    }};
}

mod unparameterized {
    use std::time::Duration;

    use crate::tests::accept_event;
    use crate::tests::setup;
    use crate::Inputs;
    use crate::Value::{self, *};
    use ordered_float::NotNan;

    use streamir_lib::ir::OutputReference;

    use crate::verdict::{Change, TotalIncremental};

    #[test]
    fn const_output_literals() {
        let spec = r#"
        input i_0: UInt8

        output o_0: Bool @i_0 := true
        output o_1: UInt8 @i_0 := 3
        output o_2: Int8 @i_0 := -5
        output o_3: Float32 @i_0 := -123.456
        "#;
        let (mut optimized, mut unoptimized) = setup(spec);
        let ts = Duration::from_secs_f64(0.0);
        let event = vec![Some(Unsigned(3))];
        let verdict = vec![
            Some(Bool(true)),
            Some(Unsigned(3)),
            Some(Signed(-5)),
            Some(Float(NotNan::try_from(-123.456).unwrap())),
        ];
        accept_event!(optimized, unoptimized, ts, event, verdict);
    }

    #[test]
    #[allow(clippy::nonminimal_bool)]
    fn const_output_arithlog() {
        let spec = r#"
        input i_0: Int8

        output o_0:   Bool @i_0 := !false
        output o_1:   Bool @i_0 := !true
        output o_2:  UInt8 @i_0 := 8 + 3
        output o_3:  UInt8 @i_0 := 8 - 3
        output o_4:  UInt8 @i_0 := 8 * 3
        output o_5:  UInt8 @i_0 := 8 / 3
        output o_6:  UInt8 @i_0 := 8 % 3
        output o_7:  UInt8 @i_0 := 8 ** 3
        output o_8:   Bool @i_0 := false || false
        output o_9:   Bool @i_0 := false || true
        output o_10:  Bool @i_0 := true  || false
        output o_11:  Bool @i_0 := true  || true
        output o_12:  Bool @i_0 := false && false
        output o_13:  Bool @i_0 := false && true
        output o_14:  Bool @i_0 := true  && false
        output o_15:  Bool @i_0 := true  && true
        output o_16:  Bool @i_0 := 0 < 1
        output o_17:  Bool @i_0 := 0 < 0
        output o_18:  Bool @i_0 := 1 < 0
        output o_19:  Bool @i_0 := 0 <= 1
        output o_20:  Bool @i_0 := 0 <= 0
        output o_21:  Bool @i_0 := 1 <= 0
        output o_22:  Bool @i_0 := 0 >= 1
        output o_23:  Bool @i_0 := 0 >= 0
        output o_24:  Bool @i_0 := 1 >= 0
        output o_25:  Bool @i_0 := 0 > 1
        output o_26:  Bool @i_0 := 0 > 0
        output o_27:  Bool @i_0 := 1 > 0
        output o_28:  Bool @i_0 := 0 == 0
        output o_29:  Bool @i_0 := 0 == 1
        output o_30:  Bool @i_0 := 0 != 0
        output o_31:  Bool @i_0 := 0 != 1
        "#;
        let (mut optimized, mut unoptimized) = setup(spec);
        let ts = Duration::from_secs_f64(0.0);
        let event = vec![Some(Unsigned(3))];
        let verdict = vec![
            Some(Bool(!false)),
            Some(Bool(!true)),
            Some(Unsigned(8 + 3)),
            Some(Unsigned(8 - 3)),
            Some(Unsigned(8 * 3)),
            Some(Unsigned(8 / 3)),
            Some(Unsigned(8 % 3)),
            Some(Unsigned(8 * 8 * 8)),
            Some(Bool(false || false)),
            Some(Bool(false || true)),
            Some(Bool(true || false)),
            Some(Bool(true || true)),
            Some(Bool(false && false)),
            Some(Bool(false && true)),
            Some(Bool(true && false)),
            Some(Bool(true && true)),
            Some(Bool(0 < 1)),
            Some(Bool(0 < 0)),
            Some(Bool(1 < 0)),
            Some(Bool(0 <= 1)),
            Some(Bool(0 <= 0)),
            Some(Bool(1 <= 0)),
            Some(Bool(0 >= 1)),
            Some(Bool(0 >= 0)),
            Some(Bool(1 >= 0)),
            Some(Bool(0 > 1)),
            Some(Bool(0 > 0)),
            Some(Bool(1 > 0)),
            Some(Bool(0 == 0)),
            Some(Bool(0 == 1)),
            Some(Bool(0 != 0)),
            Some(Bool(0 != 1)),
        ];
        accept_event!(optimized, unoptimized, ts, event, verdict);
    }

    #[test]
    fn input_only() {
        let spec = "input a: UInt8";
        let (mut optimized, mut unoptimized) = setup(spec);
        let ts = Duration::from_secs_f64(0.0);
        let event = vec![Some(Unsigned(3))];
        let verdict: Vec<Option<Value>> = vec![];
        accept_event!(optimized, unoptimized, ts, event, verdict);
    }

    #[test]
    fn sync_lookup() {
        let spec = "input a: UInt8 output b: UInt8 := a output c: UInt8 := b";
        let (mut optimized, mut unoptimized) = setup(spec);
        let ts = Duration::from_secs_f64(0.0);
        let event = vec![Some(Unsigned(9))];
        let verdict = vec![Some(Unsigned(9)), Some(Unsigned(9))];
        accept_event!(optimized, unoptimized, ts, event, verdict);
    }

    #[test]
    fn offest_hold_lookup() {
        let spec = "input a: UInt8\n\
        output b := a.offset(by: -1).defaults(to: 3)\n\
        output x: UInt8 @5Hz := b.hold().defaults(to: 3)";
        let (mut optimized, mut unoptimized) = setup(spec);
        let ts = Duration::from_secs_f64(0.0);
        let event = vec![Some(Unsigned(9))];
        let verdict = vec![Some(Unsigned(3)), Option::None];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(0.1);
        let event = vec![Some(Unsigned(5))];
        let verdict = vec![Some(Unsigned(9)), Option::None];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(0.3);
        let event = vec![Some(Unsigned(7))];
        let exp_verdict = vec![Some(Unsigned(5)), Option::None];
        let exp_timed_ts = Duration::from_secs_f64(0.2);
        let exp_timed_verdict = vec![Option::None, Some(Unsigned(9))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            exp_verdict,
            exp_timed_ts,
            exp_timed_verdict
        )
    }

    #[test]
    fn get_fresh_hold_lookup() {
        let spec = "input a: UInt8\n\
        input b: UInt8\n\
        output x @b := a.get().defaults(to: 9)\n\
        output y @b := a.is_fresh()\n\
        output z @a := b.hold(or: 5)";
        let (mut optimized, mut unoptimized) = setup(spec);
        let ts = Duration::from_secs_f64(0.0);
        let event = vec![Some(Unsigned(3)), Option::None];
        let verdict = vec![Option::None, Option::None, Some(Unsigned(5))];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(0.1);
        let event = vec![Option::None, Some(Unsigned(2))];
        let verdict = vec![Some(Unsigned(9)), Some(Bool(false)), Option::None];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(0.1);
        let event = vec![Some(Unsigned(1)), Some(Unsigned(2))];
        let verdict = vec![Some(Unsigned(1)), Some(Bool(true)), Some(Unsigned(2))];
        accept_event!(optimized, unoptimized, ts, event, verdict);
    }

    #[test]
    fn sliding_windows() {
        let spec = "input a: Int16\n\
        input b: Float64\n\
        output sum: Int16 @0.25Hz := a.aggregate(over: 10s, using: sum)
        output count: UInt16 @0.25Hz := a.aggregate(over: 10s, using: count)
        output avg @0.25Hz := b.aggregate(over: 10s, using: avg).defaults(to: -3.0)";
        let (mut optimized, mut unoptimized) = setup(spec);
        let mut ts = Duration::from_secs_f64(0.1);
        let mut values = Vec::new();
        for v in 1..=25_i64 {
            let event = vec![
                Some(Signed(v)),
                Some(Float(NotNan::<f64>::new(v as f64).unwrap())),
            ];
            let exp_verdict = vec![Option::None, Option::None, Option::None];
            if v % 4 == 1 && v != 1 {
                let periodic_time = Duration::from_secs(ts.as_secs());
                let filter_values = values
                    .iter()
                    .filter(|(v_ts, _)| *v_ts + Duration::from_secs(10) > periodic_time);
                let sum = filter_values.clone().fold(0, |sum, (_ts, v)| sum + v);
                let count = filter_values.clone().count();
                let avg = sum as f64 / count as f64;
                accept_event!(
                    optimized,
                    unoptimized,
                    ts,
                    event,
                    exp_verdict,
                    periodic_time,
                    vec![
                        Some(Signed(sum)),
                        Some(Unsigned(count as u64)),
                        Some(Float(NotNan::new(avg).unwrap()))
                    ]
                )
            } else {
                accept_event!(optimized, unoptimized, ts, event, exp_verdict);
            }
            values.push((ts, v));
            ts += Duration::from_secs(1);
        }
    }

    #[test]
    fn sliding_windows_2() {
        let spec = "input a: Float64\n\
        output count @1Hz := a.aggregate(over: 5s, using: count)";
        let (mut optimized, mut unoptimized) = setup(spec);
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.0),
            vec![Some(Float(NotNan::from(1)))],
            vec![]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(1.0),
            vec![Some(Float(NotNan::from(2)))],
            vec![]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(2.0),
            vec![Some(Float(NotNan::from(3)))],
            vec![],
            Duration::from_secs_f64(1.0),
            vec![Some(Unsigned(2))]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(3.0),
            vec![Some(Float(NotNan::from(4)))],
            vec![],
            Duration::from_secs_f64(2.0),
            vec![Some(Unsigned(3))]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(4.0),
            vec![Some(Float(NotNan::from(5)))],
            vec![],
            Duration::from_secs_f64(3.0),
            vec![Some(Unsigned(4))]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(5.0),
            vec![Some(Float(NotNan::from(6)))],
            vec![],
            Duration::from_secs_f64(4.0),
            vec![Some(Unsigned(5))]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(6.0),
            vec![Some(Float(NotNan::from(7)))],
            vec![],
            Duration::from_secs_f64(5.0),
            vec![Some(Unsigned(5))]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(7.0),
            vec![Some(Float(NotNan::from(8)))],
            vec![],
            Duration::from_secs_f64(6.0),
            vec![Some(Unsigned(5))]
        );
    }

    #[test]
    fn integral() {
        let spec = "input a: Float64\n\
        output count @1Hz := a.aggregate(over: 5s, using: integral)";
        let (mut optimized, mut unoptimized) = setup(spec);
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.0),
            vec![Some(Float(NotNan::from(0)))],
            vec![]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(1.0),
            vec![Some(Float(NotNan::from(8)))],
            vec![]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(1.1),
            vec![Some(Float(NotNan::from(8)))],
            vec![],
            Duration::from_secs_f64(1.0),
            vec![Some(Float(NotNan::new(4.0).unwrap()))]
        );
    }

    #[test]
    fn integral_over_exactly() {
        let spec = "input a: Float64\n\
        output count @0.25Hz := a.aggregate(over_exactly: 40s, using: integral).defaults(to: -3.0)";
        let (mut optimized, mut unoptimized) = setup(spec);
        let mut time = Duration::from_secs_f64(0.1);
        for i in 1..10 {
            time += Duration::from_secs(4);
            accept_event!(
                optimized,
                unoptimized,
                time,
                vec![Option::None],
                vec![],
                Duration::from_secs_f64(4.0) * i,
                vec![Some(Float(NotNan::from(-3)))]
            );
        }
        time += Duration::from_secs(4);
        accept_event!(
            optimized,
            unoptimized,
            time,
            vec![Option::None],
            vec![],
            Duration::from_secs_f64(40.0),
            vec![Some(Float(NotNan::from(0)))]
        );
        time = Duration::from_secs_f64(45.0);
        accept_event!(
            optimized,
            unoptimized,
            time,
            vec![Some(Float(NotNan::from(1)))],
            vec![],
            Duration::from_secs_f64(44.0),
            vec![Some(Float(NotNan::from(0)))]
        );
        time += Duration::from_secs(2);
        accept_event!(
            optimized,
            unoptimized,
            time,
            vec![Some(Float(NotNan::from(5)))],
            vec![]
        );
        time += Duration::from_secs(5);
        accept_event!(
            optimized,
            unoptimized,
            time,
            vec![Some(Float(NotNan::from(25)))],
            vec![],
            Duration::from_secs_f64(48.0),
            vec![Some(Float(NotNan::from(6)))]
        );
        time += Duration::from_secs(1);
        accept_event!(
            optimized,
            unoptimized,
            time,
            vec![Some(Float(NotNan::from(0)))],
            vec![],
            Duration::from_secs_f64(52.0),
            vec![Some(Float(NotNan::from(81)))]
        );
        time += Duration::from_secs(4);
        accept_event!(
            optimized,
            unoptimized,
            time,
            vec![Option::None],
            vec![],
            Duration::from_secs_f64(56.0),
            vec![Some(Float(NotNan::new(93.5).unwrap()))]
        );
        time += Duration::from_secs(4);
        accept_event!(
            optimized,
            unoptimized,
            time,
            vec![Option::None],
            vec![],
            Duration::from_secs_f64(60.0),
            vec![Some(Float(NotNan::new(93.5).unwrap()))]
        );
        time += Duration::from_secs(2);
        accept_event!(
            optimized,
            unoptimized,
            time,
            vec![Some(Float(NotNan::from(-40)))],
            vec![]
        );
        time += Duration::from_secs(2);
        accept_event!(
            optimized,
            unoptimized,
            time,
            vec![Option::None],
            vec![],
            Duration::from_secs_f64(64.0),
            vec![Some(Float(NotNan::new(-106.5).unwrap()))]
        );
    }

    #[test]
    fn spawn_eval_close() {
        let spec = "input a: UInt8\n\
        output b spawn when a > 5 eval with a + b.offset(by: -1).defaults(to: 0) close when b > 10\n\
        output c @a := b.hold(or: 20) + 3";
        let (mut optimized, mut unoptimized) = setup(spec);
        let ts = Duration::from_secs_f64(0.0);
        let event = vec![Some(Unsigned(3))];
        let verdict = vec![Option::None, Some(Unsigned(23))];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(0.1);
        let event = vec![Some(Unsigned(6))];
        let verdict = vec![Some(Unsigned(6)), Some(Unsigned(9))];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(0.3);
        let event = vec![Some(Unsigned(1))];
        let verdict = vec![Some(Unsigned(7)), Some(Unsigned(10))];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(0.4);
        let event = vec![Some(Unsigned(4))];
        let verdict = vec![Some(Unsigned(11)), Some(Unsigned(14))];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(0.5);
        let event = vec![Some(Unsigned(2))];
        let verdict = vec![Option::None, Some(Unsigned(23))];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(0.6);
        let event = vec![Some(Unsigned(6))];
        let verdict = vec![Some(Unsigned(6)), Some(Unsigned(9))];
        accept_event!(optimized, unoptimized, ts, event, verdict);
    }

    #[test]
    fn dynamic_deadlines() {
        let spec = "input a: UInt8\n\
        output b spawn when a > 5 eval @1Hz with a.hold(or: 20) + b.offset(by: -1).defaults(to: 0) close when a > 10\n\
        output c spawn when a > 5 eval @0.5Hz with b close when a > 10";
        let (mut optimized, mut unoptimized) = setup(spec);
        let ts = Duration::from_secs_f64(0.1);
        let event = vec![Some(Unsigned(3))];
        let verdict = vec![Option::None, Option::None];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(2.1);
        let event = vec![Some(Unsigned(4))];
        let verdict = vec![Option::None, Option::None];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(2.2);
        let event = vec![Some(Unsigned(6))];
        let verdict = vec![Option::None, Option::None];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(3.3);
        let event = vec![Some(Unsigned(9))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            vec![Option::None, Option::None],
            Duration::from_secs_f64(3.2),
            vec![Some(Unsigned(6)), Option::None,]
        );
        let ts = Duration::from_secs_f64(4.3);
        let event = vec![Some(Unsigned(3))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            vec![Option::None, Option::None],
            Duration::from_secs_f64(4.2),
            vec![Some(Unsigned(15)), Some(Unsigned(15))]
        );
        let ts = Duration::from_secs_f64(5.3);
        let event = vec![Some(Unsigned(3))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            vec![Option::None, Option::None],
            Duration::from_secs_f64(5.2),
            vec![Some(Unsigned(18)), Option::None]
        );
        let ts = Duration::from_secs_f64(5.5);
        let event = vec![Some(Unsigned(11))];
        let verdict = vec![Option::None, Option::None];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(6.3);
        let event = vec![Some(Unsigned(10))];
        let verdict = vec![Option::None, Option::None];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(7.4);
        let event = vec![Some(Unsigned(3))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            vec![Option::None, Option::None],
            Duration::from_secs_f64(7.3),
            vec![Some(Unsigned(10)), Option::None]
        );
    }

    #[test]
    #[ignore = "not yet implemented"]
    fn close_eval_different_frequencies() {
        let spec = "input a: UInt8\n\
        output b spawn when a > 5 eval @Local(1Hz) with a.hold(or: 20) + b.offset(by: -1).defaults(to: 0) close @Local(0.5Hz) when a.hold(or: 0) > 10";
        let (mut optimized, mut unoptimized) = setup(spec);
        let ts = Duration::from_secs_f64(0.1);
        let event = vec![Some(Unsigned(3))];
        let verdict = vec![Option::None];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(0.2);
        let event = vec![Some(Unsigned(13))];
        let verdict = vec![Option::None];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(1.3);
        let event = vec![Some(Unsigned(5))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            vec![Option::None, Option::None],
            Duration::from_secs_f64(1.2),
            vec![Some(Unsigned(13))]
        );
        let ts = Duration::from_secs_f64(2.3);
        let event = vec![Some(Unsigned(6))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            vec![Option::None],
            Duration::from_secs_f64(2.2),
            vec![Some(Unsigned(18))]
        );
        let ts = Duration::from_secs_f64(3.3);
        let event = vec![Some(Unsigned(16))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            vec![Option::None],
            Duration::from_secs_f64(2.2),
            vec![Some(Unsigned(24))]
        );
        let ts = Duration::from_secs_f64(4.3);
        let event = vec![Some(Unsigned(6))];
        accept_event!(optimized, unoptimized, ts, event, vec![Option::None]);
    }

    #[test]
    fn spawn_global_freg() {
        let spec = "input a: UInt8\n\
        output b spawn @1Hz when a.hold(or: 0) > 5 eval @Global(2Hz) with a.hold(or: 20) + b.offset(by: -1).defaults(to: 0)";
        let (mut optimized, mut unoptimized) = setup(spec);
        let ts = Duration::from_secs_f64(0.1);
        let event = vec![Some(Unsigned(3))];
        let verdict = vec![Option::None];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(1.1);
        let event = vec![Some(Unsigned(6))];
        let verdict = vec![Option::None];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(2.1);
        let event = vec![Some(Unsigned(6))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            vec![Option::None],
            Duration::from_secs_f64(2.0),
            vec![Some(Unsigned(6))]
        );
        let ts = Duration::from_secs_f64(2.6);
        let event = vec![Some(Unsigned(7))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            vec![Option::None],
            Duration::from_secs_f64(2.5),
            vec![Some(Unsigned(12))]
        );
        let ts = Duration::from_secs_f64(3.1);
        let event = vec![Some(Unsigned(8))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            vec![Option::None],
            Duration::from_secs_f64(3.0),
            vec![Some(Unsigned(19))]
        );
    }

    #[test]
    fn global_and_local_frequencies() {
        let spec = "input a: UInt8\n\
        output b spawn when a > 5 eval @Local(1Hz) with a.hold(or: 20) + b.offset(by: -1).defaults(to: 0)\n\
        output c spawn when a > 5 eval @Global(1Hz) with a.hold(or: 20) + c.offset(by: -1).defaults(to: 0)";
        let (mut optimized, mut unoptimized) = setup(spec);
        let ts = Duration::from_secs_f64(0.3);
        let event = vec![Some(Unsigned(6))];
        let verdict = vec![Option::None, Option::None];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(1.2);
        let event = vec![Some(Unsigned(8))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            vec![Option::None, Option::None],
            Duration::from_secs_f64(1.0),
            vec![Option::None, Some(Unsigned(6))]
        );
        let ts = Duration::from_secs_f64(1.4);
        let event = vec![Some(Unsigned(10))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            vec![Option::None, Option::None],
            Duration::from_secs_f64(1.3),
            vec![Some(Unsigned(8)), Option::None]
        );
    }

    #[test]
    fn global_and_local_windows() {
        let spec = "input a: UInt8\n\
        output b spawn when a > 5 eval @Local(1Hz) with a.aggregate(over: 2s, using: count)\n\
        output c spawn when a > 5 eval @Global(1Hz) with a.aggregate(over: 2s, using: count)";
        let (mut optimized, mut unoptimized) = setup(spec);
        let ts = Duration::from_secs_f64(0.3);
        let event = vec![Some(Unsigned(6))];
        let verdict = vec![Option::None, Option::None];
        accept_event!(optimized, unoptimized, ts, event, verdict);
        let ts = Duration::from_secs_f64(1.2);
        let event = vec![Some(Unsigned(8))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            vec![Option::None, Option::None],
            Duration::from_secs_f64(1.0),
            vec![Option::None, Some(Unsigned(1))]
        );
        let ts = Duration::from_secs_f64(1.4);
        let event = vec![Some(Unsigned(10))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            vec![Option::None, Option::None],
            Duration::from_secs_f64(1.3),
            vec![Some(Unsigned(2)), Option::None]
        );
        let ts = Duration::from_secs_f64(2.2);
        let event = vec![Some(Unsigned(8))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            vec![Option::None, Option::None],
            Duration::from_secs_f64(2.0),
            vec![Option::None, Some(Unsigned(3))]
        );
        let ts = Duration::from_secs_f64(2.4);
        let event = vec![Some(Unsigned(10))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            vec![Option::None, Option::None],
            Duration::from_secs_f64(2.3),
            vec![Some(Unsigned(3)), Option::None]
        );
        let ts = Duration::from_secs_f64(3.2);
        let event = vec![Some(Unsigned(10))];
        accept_event!(
            optimized,
            unoptimized,
            ts,
            event,
            vec![Option::None, Option::None],
            Duration::from_secs_f64(3.0),
            vec![Option::None, Some(Unsigned(4))]
        );
    }
}

mod parameterized {
    use std::time::Duration;

    use crate::tests::accept_event;
    use crate::tests::setup;
    use crate::value::Value;
    use crate::verdict::Change;
    use crate::verdict::TotalIncremental;
    use crate::Inputs;
    use crate::Value::*;
    use streamir_lib::ir::OutputReference::*;

    macro_rules! pverdict {
        ($(($($($p:expr),+ => $v:expr),*)),*)  => {
            vec![$(vec![$((vec![$($p),+], $v)),*]),*]
        };
    }

    #[test]
    fn spawn_eval_close() {
        let spec = r#"
        input i: UInt8

        output o (p): UInt8
            spawn with i
            eval when i = p with o(p).last(or: 0) + 1
            close when o(p) == 2
        output o_1 @i := o(1).hold(or: 100)
        output o_2 @i := o(2).hold(or: 100)
        output o_3 @i := o(3).hold(or: 100)
        "#;

        let (mut optimized, mut unoptimized) = setup(spec);
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.0),
            vec![Some(Unsigned(3))],
            vec![Some(Unsigned(100)), Some(Unsigned(100)), Some(Unsigned(1))],
            pverdict![
                (Unsigned(3) => Unsigned(1))
            ]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.0),
            vec![Some(Unsigned(2))],
            vec![Some(Unsigned(100)), Some(Unsigned(1)), Some(Unsigned(1))],
            pverdict![
                (Unsigned(2) => Unsigned(1))
            ]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.0),
            vec![Some(Unsigned(3))],
            vec![Some(Unsigned(100)), Some(Unsigned(1)), Some(Unsigned(2))],
            pverdict![
                (Unsigned(3) => Unsigned(2))
            ]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.0),
            vec![Some(Unsigned(3))],
            vec![Some(Unsigned(100)), Some(Unsigned(1)), Some(Unsigned(1))],
            pverdict![
                (Unsigned(3) => Unsigned(1))
            ]
        );
    }

    #[test]
    fn spawn_eval_close2() {
        let spec = r#"
        input i: UInt8

        output o (p): UInt8
            spawn with i
            eval  @i with o(p).last(or: 0) + 1
            close when o(p) == 3

        output o_1 @i := o(1).hold(or: 100)
        output o_2 @i := o(2).hold(or: 100)
        output o_3 @i := o(3).hold(or: 100)
        "#;

        let (mut optimized, mut unoptimized) = setup(spec);
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.0),
            vec![Some(Unsigned(3))],
            vec![Some(Unsigned(100)), Some(Unsigned(100)), Some(Unsigned(1))],
            pverdict![
                (Unsigned(3) => Unsigned(1))
            ]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.0),
            vec![Some(Unsigned(2))],
            vec![Some(Unsigned(100)), Some(Unsigned(1)), Some(Unsigned(2))],
            pverdict![
                (Unsigned(2) => Unsigned(1), Unsigned(3) => Unsigned(2))
            ]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.0),
            vec![Some(Unsigned(3))],
            vec![Some(Unsigned(100)), Some(Unsigned(2)), Some(Unsigned(3))],
            pverdict![
                (Unsigned(2) => Unsigned(2), Unsigned(3) => Unsigned(3))
            ]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.0),
            vec![Some(Unsigned(3))],
            vec![Some(Unsigned(100)), Some(Unsigned(3)), Some(Unsigned(1))],
            pverdict![
                (Unsigned(2) => Unsigned(3), Unsigned(3) => Unsigned(1))
            ]
        );
    }

    #[test]
    fn dynamic_periodic() {
        let spec = r#"
        input i: UInt8

        output o (p): UInt8
            spawn with i
            eval  @1Hz with o(p).last(or: 0) + 1
            close when i == 10
        
        output o' (p): UInt8
            spawn with i
            eval when i == p with o(3).hold(or: 100)

        output o''(p): UInt8
            spawn with i
            eval @2s with o''(p).last(or: 0) + o(p)
            close when i == 10
        "#;

        let (mut optimized, mut unoptimized) = setup(spec);
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.2),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(), (Unsigned(3) => Unsigned(100)), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.4),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(), (Unsigned(3) => Unsigned(100)), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.6),
            vec![Some(Unsigned(2))],
            vec![],
            pverdict![(), (Unsigned(2) => Unsigned(100)), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(1.3),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(), (Unsigned(3) => Unsigned(1)), ()],
            Duration::from_secs_f64(1.2),
            vec![],
            pverdict![(Unsigned(3) => Unsigned(1)), (), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(1.7),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(), (Unsigned(3) => Unsigned(1)), ()],
            Duration::from_secs_f64(1.6),
            vec![],
            pverdict![(Unsigned(2) => Unsigned(1)), (), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(2.4),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(), (Unsigned(3) => Unsigned(2)), ()],
            Duration::from_secs_f64(2.2),
            vec![],
            pverdict![(Unsigned(3) => Unsigned(2)), (), (Unsigned(3) => Unsigned(2))]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(3.0),
            vec![Some(Unsigned(10))],
            vec![],
            pverdict![(), (Unsigned(10) => Unsigned(2)), ()],
            Duration::from_secs_f64(2.6),
            vec![],
            pverdict![(Unsigned(2) => Unsigned(2)), (), (Unsigned(2) => Unsigned(2))]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(3.7),
            vec![Some(Unsigned(10))],
            vec![],
            pverdict![(), (Unsigned(10) => Unsigned(100)), ()]
        );
    }

    #[test]
    fn global_periodic() {
        let spec = r#"
        input i: UInt8

        output o (p): UInt8
            spawn with i
            eval  @Global(1Hz) with o(p).last(or: 0) + 1
            close when i == 10
        
        output o' (p): UInt8
            spawn with i
            eval when i == p with o(3).hold(or: 100)

        output o''(p): UInt8
            spawn with i
            eval @Global(2s) with o''(p).last(or: 0) + o(p)
            close when i == 10
        "#;

        let (mut optimized, mut unoptimized) = setup(spec);
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.2),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(), (Unsigned(3) => Unsigned(100)), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.4),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(), (Unsigned(3) => Unsigned(100)), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.6),
            vec![Some(Unsigned(2))],
            vec![],
            pverdict![(), (Unsigned(2) => Unsigned(100)), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(1.3),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(), (Unsigned(3) => Unsigned(1)), ()],
            Duration::from_secs_f64(1.0),
            vec![],
            pverdict![(Unsigned(2) => Unsigned(1), Unsigned(3) => Unsigned(1)), (), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(1.7),
            vec![Some(Unsigned(4))],
            vec![],
            pverdict![(), (Unsigned(4) => Unsigned(1)), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(2.4),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(), (Unsigned(3) => Unsigned(2)), ()],
            Duration::from_secs_f64(2.0),
            vec![],
            pverdict![(Unsigned(2) => Unsigned(2), Unsigned(3) => Unsigned(2), Unsigned(4) => Unsigned(1)), (), (Unsigned(2) => Unsigned(2), Unsigned(3) => Unsigned(2), Unsigned(4) => Unsigned(1))]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(3.0),
            vec![Some(Unsigned(10))],
            vec![],
            pverdict![(), (Unsigned(10) => Unsigned(2)), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(3.7),
            vec![Some(Unsigned(10))],
            vec![],
            pverdict![(), (Unsigned(10) => Unsigned(100)), ()]
        );
    }

    #[test]
    #[ignore = "unimplemented: Windowaggregtation in parameterized stream to unparamerterized stream"]
    fn windows_1() {
        let spec = r#"
        input i: UInt8

        output global (p): UInt8
            spawn with i
            eval  @Global(1Hz) with i.aggregate(over: 2s, using: sum)
            close when i == 10
        
        output local (p): UInt8
            spawn with i
            eval @1Hz with i.aggregate(over: 2s, using: sum)
            close when i == 10
        "#;

        let (mut optimized, mut unoptimized) = setup(spec);
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.2),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.2),
            vec![Some(Unsigned(1))],
            vec![],
            pverdict![(), ()]
        );
    }

    #[test]
    fn windows_2() {
        let spec = r#"
        input i: UInt8

        output o(p): UInt8
            spawn with i
            eval when p = i with o(p).last(or: 0) + 1

        output global (p): UInt8
            spawn with i
            eval  @Global(1Hz) with o(p).aggregate(over: 2s, using: sum)
            close when i == 10
        
        output local (p): UInt8
            spawn with i
            eval @1Hz with o(p).aggregate(over: 2s, using: sum)
            close when i == 10
        "#;

        let (mut optimized, mut unoptimized) = setup(spec);
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.2),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(Unsigned(3) => Unsigned(1)), (), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.4),
            vec![Some(Unsigned(4))],
            vec![],
            pverdict![(Unsigned(4) => Unsigned(1)), (), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.6),
            vec![Some(Unsigned(4))],
            vec![],
            pverdict![(Unsigned(4) => Unsigned(2)), (), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.7),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(Unsigned(3) => Unsigned(2)), (), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(1.0),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(Unsigned(3) => Unsigned(3)), (), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(1.2),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(Unsigned(3) => Unsigned(4)), (), ()],
            Duration::from_secs_f64(1.0),
            vec![],
            pverdict![(), (Unsigned(3) => Unsigned(6), Unsigned(4) => Unsigned(3)), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(1.4),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(Unsigned(3) => Unsigned(5)), (), ()],
            Duration::from_secs_f64(1.2),
            vec![],
            pverdict![(), (), (Unsigned(3) => Unsigned(10))]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(1.6),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(Unsigned(3) => Unsigned(6)), (), ()],
            Duration::from_secs_f64(1.4),
            vec![],
            pverdict![(), (), (Unsigned(4) => Unsigned(3))]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(2.1),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(Unsigned(3) => Unsigned(7)), (), ()],
            Duration::from_secs_f64(2.0),
            vec![],
            pverdict![(), (Unsigned(3) => Unsigned(21), Unsigned(4) => Unsigned(3)), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(2.3),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(Unsigned(3) => Unsigned(8)), (), ()],
            Duration::from_secs_f64(2.2),
            vec![],
            pverdict![(), (), (Unsigned(3) => Unsigned(27))]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(2.8),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(Unsigned(3) => Unsigned(9)), (), ()],
            Duration::from_secs_f64(2.4),
            vec![],
            pverdict![(), (), (Unsigned(4) => Unsigned(2))]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(3.1),
            vec![Some(Unsigned(10))],
            vec![],
            pverdict![(Unsigned(10) => Unsigned(1)), (), ()],
            Duration::from_secs_f64(3.0),
            vec![],
            pverdict![(), (Unsigned(3) => Unsigned(39), Unsigned(4) => Unsigned(0)), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(3.6),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(Unsigned(3) => Unsigned(10)), (), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(4.3),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(Unsigned(3) => Unsigned(11)), (), ()],
            Duration::from_secs_f64(4.0),
            vec![],
            pverdict![(), (Unsigned(3) => Unsigned(10)), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(4.7),
            vec![Some(Unsigned(3))],
            vec![],
            pverdict![(Unsigned(3) => Unsigned(12)), (), ()],
            Duration::from_secs_f64(4.6),
            vec![],
            pverdict![(), (), (Unsigned(3) => Unsigned(21))]
        );
    }

    #[test]
    fn instance_windows() {
        let spec = r#"
        input i: UInt8

        output o(p): UInt8
            spawn with i
            eval when p >= i with o(p).last(or: 0) + 1

        output unfiltered_all: UInt8 @i := o.aggregate(over_instances: all, using: sum)
        output filtered_all: UInt8 @i := o.aggregate(over_instances: all(where: p => p > 2), using: sum)
        output unfiltered_fresh: UInt8 @i := o.aggregate(over_instances: fresh, using: sum)
        output filtered_fresh: UInt8 @i := o.aggregate(over_instances: fresh(where: p => p > 2), using: sum)
        "#;

        let (mut optimized, mut unoptimized) = setup(spec);
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.2),
            vec![Some(Unsigned(3))],
            vec![
                Some(Unsigned(1)),
                Some(Unsigned(1)),
                Some(Unsigned(1)),
                Some(Unsigned(1))
            ],
            pverdict![(Unsigned(3) => Unsigned(1)), (), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.3),
            vec![Some(Unsigned(2))],
            vec![
                Some(Unsigned(3)),
                Some(Unsigned(2)),
                Some(Unsigned(3)),
                Some(Unsigned(2))
            ],
            pverdict![(Unsigned(2) => Unsigned(1), Unsigned(3) => Unsigned(2)), (), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.3),
            vec![Some(Unsigned(4))],
            vec![
                Some(Unsigned(4)),
                Some(Unsigned(3)),
                Some(Unsigned(1)),
                Some(Unsigned(1))
            ],
            pverdict![(Unsigned(4) => Unsigned(1)), (), ()]
        );
        accept_event!(
            optimized,
            unoptimized,
            Duration::from_secs_f64(0.3),
            vec![Some(Unsigned(2))],
            vec![
                Some(Unsigned(7)),
                Some(Unsigned(5)),
                Some(Unsigned(7)),
                Some(Unsigned(5))
            ],
            pverdict![(Unsigned(4) => Unsigned(2), Unsigned(3) => Unsigned(3), Unsigned(2) => Unsigned(2)), (), ()]
        );
    }
}
