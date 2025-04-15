impl Event {
    fn from_csv(line: &str) -> (Self, Duration) {
        let mut values = line.split(",");

		{%- for input in inputs %}
        {%- if not input.name == "time" %}
        let {{ input.name }}_str = values.next().expect("expecting value for input {{ input.name }}").trim();
        let {{ input.name }} = ({{ input.name }}_str != "#").then(|| {{ input.parse_code }});
        {%- endif %}
		{%- endfor %}

        let time_str = values.next().expect("expecting value for time").trim();
        let time = Duration::from_secs_f64(time_str.parse::<f64>().expect("error parsing time"));

        (Event { {% for input in inputs %}{%- if not input.name == "time" %}{{ input.name }},{% endif %}{% endfor %} }, time)
    }
}

fn main() {
    let trace_path = std::env::args().nth(1).expect("Give trace as first argument");
    let trace_file = File::open(trace_path).unwrap();
    let mut csv_lines = BufReader::new(trace_file).lines().skip(1); // skip header file

    {%- if not silent %}
    {{ verdict_header_function }};
    {%- endif %}

    let first_line = csv_lines.next().unwrap_or_else(|| exit(0)).unwrap();
    let (first_event, start_time) = Event::from_csv(&first_line);

    let mut monitor = Monitor::new(start_time);
    let verdicts = monitor
        .accept_event(first_event, start_time)
        .expect("error monitoring event");
    verdicts.iter().for_each(|v| print!("{v}"));

    let mut current_time = Duration::new(0,0);
    for csv_line in csv_lines.map(|line| line.unwrap()) {
        let (event, time) = Event::from_csv(&csv_line);
        let verdicts = monitor.accept_event(event, time).expect("error monitoring event");
        unsafe {
        std::mem::forget(ptr::read_volatile(&verdicts));
        }
        {%- if not silent %}
        verdicts.iter().for_each(|v| print!("{v}"));
        {%- endif %}
        current_time = time;
    }
    let verdicts = monitor.close(current_time).expect("error closing monitor");
    {%- if not silent %}
    verdicts.iter().for_each(|v| print!("{v}"));
    {%- endif %}
    unsafe {
    std::mem::forget(ptr::read_volatile(&verdicts));
    }
}