#[derive(Debug, Clone, PartialEq, Eq)]
struct State {
    time: Duration,
    deadline: Deadline
}

impl State {
    fn new_after(deadline: Deadline, time: Duration) -> Self {
        match deadline {
            {%- for deadline in deadlines %}
            Deadline::{{ deadline.1 }}{% if deadline.2 %}(_){% endif %} => State { time: time + {{ deadline.0 }}, deadline },
            {%- endfor %}
        }
    }
}

impl Ord for State {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.time.cmp(&self.time)
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone)]
struct Queue(std::collections::BinaryHeap<State>);

impl Queue {
    fn new(start_time: Duration) -> Self {
        {%- for deadline in deadlines %}{% if deadline.3 %}
        let state{{ loop.index0 }} = State::new_after(Deadline::{{deadline.1}}, start_time);
        {%- endif %}{% endfor %}
        Self(std::collections::BinaryHeap::from(vec![{%- for deadline in deadlines %}{% if deadline.3 %}state{{ loop.index0 }}, {% endif %}{%- endfor %}]))
    }

    fn push(&mut self, item: State) {
        self.0.push(item)
    }

    fn pop(&mut self) -> Option<State> {
        self.0.pop()
    }

    {%- if has_dynamic_periodic %}
    fn collect_and_add(&mut self, mut spawned_streams: Vec<Deadline>, time: Duration) {
        spawned_streams.sort();
        let deadlines = spawned_streams.into_iter().fold(Vec::new(), |mut acc, deadline| {
            if let Some(last) = acc.last_mut() {
                match (last, deadline) {
                    {%- for deadline in deadlines %}{% if deadline.2 %}
                    {% if not loop.first %}| {% endif %}(Deadline::{{ deadline.1 }}(ref mut last), Deadline::{{ deadline.1 }}(deadline))
                    {%- endif %}{% endfor %} => last.extend(deadline),
                    (_, _) => {}
                };
            } else {
                acc.push(deadline);
            }
            acc
        });
        self.0.extend(deadlines.into_iter().map(|deadline| State::new_after(deadline, time)))
    }

    fn remove(&mut self, closed_streams: Vec<StreamReference>) {
        if !closed_streams.is_empty() {
            self.0 = self
                .0
                .drain()
                .filter_map(|State { time, deadline }| {
                    match deadline {
                        {%- if has_dynamic_periodic %}
                        {%- for deadline in deadlines %}{% if deadline.2 %}
                        Deadline::{{ deadline.1 }}(mut streams) => {
                            streams.retain(|sr| !closed_streams.contains(sr));
                            if streams.is_empty() {
                                None
                            } else {
                                Some(State {
                                    time,
                                    deadline: Deadline::{{ deadline.1 }}(streams),
                                })
                            }
                        },
                        {%- endif %}
                        {%- endfor %}
                        {%- endif %}
                        {%- if has_static_periodic %}
                        _ => Some(State {time, deadline})
                        {%- endif %}
                    }
                })
                .collect();
        }
    }
    {%- endif %}

    // fn next(&mut self, end: Duration) -> Option<Internal> {
    //     while let Some(state) = self.pop() {
    //         if state.time >= end {
    //             self.push(state);
    //             return None;
    //         }
    //         let State { time, deadline } = state;
    //         let event = Internal::new_periodic_event(&time, &deadline);
    //         let new_state = State::new_after(deadline, time);
    //         self.push(new_state);
    //         Some(event)
    //     } else {
    //         None
    //     }
    // }

    fn next(&mut self, end: Duration, inclusive: bool) -> Option<Internal> {
        let mut current: Option<Internal> = None;
        while let Some(state) = self.pop() {
            if (!inclusive && state.time >= end) || state.time > end {
                self.push(state);
                return current;
            }

            if let Some(current_event) = &current {
                if state.time > current_event.event_time {
                    self.push(state);
                    return current;
                }
            }

            let State { time, deadline } = state;

            let current_event = current.get_or_insert_with(|| Internal::empty(state.time));
            match &deadline {
                {%- for deadline in deadlines %}
                Deadline::{{ deadline.1 }}{% if deadline.2 %}(v){% endif %} => {current_event.{{ deadline.1 | lower }}{% if deadline.2 %}.extend(v){% else %} = true{% endif %};}
                {%- endfor %}
            }
            let new_state = State::new_after(deadline, time);
            self.push(new_state);
        }
        current
    }
}
