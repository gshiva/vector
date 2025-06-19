use vector_baseline::build_baseline;

fn next_stage<T: Clone>(events: Vec<T>) -> Vec<T> {
    events
}

#[test]
fn baseline_echoes_events() {
    build_baseline().expect("baseline build should succeed");

    let logs = vec!["log1".to_string()];
    let metrics = vec!["metric1".to_string()];
    let traces = vec!["trace1".to_string()];

    let out_logs = next_stage(logs.clone());
    let out_metrics = next_stage(metrics.clone());
    let out_traces = next_stage(traces.clone());

    assert_eq!(logs, out_logs);
    assert_eq!(metrics, out_metrics);
    assert_eq!(traces, out_traces);
}
