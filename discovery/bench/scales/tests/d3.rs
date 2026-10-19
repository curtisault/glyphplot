//! Compares the hand-written port (src/hand.rs) with d3's outputs captured by
//! fixtures/capture.mjs. Prints the mismatches and the counts; fails on any.
use scales_bench::hand;
use serde_json::Value;

fn nums(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()
}

#[test]
fn matches_d3() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/d3.jsonl")).unwrap();
    let (mut cases, mut tick_bad, mut nice_bad, mut label_bad) = (0, 0, 0, 0);
    for line in text.lines() {
        let c: Value = serde_json::from_str(line).unwrap();
        let (a, b, n) = (c["a"].as_f64().unwrap(), c["b"].as_f64().unwrap(), c["n"].as_f64().unwrap());
        let want_ticks = nums(&c["ticks"]);
        let want_labels: Vec<String> = c["labels"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect();
        cases += 1;
        let (ticks, labels): (Vec<f64>, Vec<String>) = if c["kind"] == "linear" {
            let f = hand::SiFormat::new(a, b, n);
            let t = hand::ticks(a, b, n);
            let l = t.iter().map(|&x| { let mut s = String::new(); f.format(x, &mut s); s }).collect();
            let want_nice = nums(&c["nice"]);
            let (n0, n1) = hand::nice(a, b, n);
            if [n0, n1] != want_nice[..] {
                nice_bad += 1;
                eprintln!("nice {a} {b} {n}: got [{n0}, {n1}] want {want_nice:?}");
            }
            (t, l)
        } else {
            let t = hand::utc_ticks(a, b, n);
            let l = t.iter().map(|&x| { let mut s = String::new(); hand::utc_label(x, &mut s); s }).collect();
            (t, l)
        };
        if ticks != want_ticks {
            tick_bad += 1;
            eprintln!("{} ticks {a} {b} {n}: got {ticks:?} want {want_ticks:?}", c["kind"]);
        } else if labels != want_labels {
            label_bad += 1;
            eprintln!("{} labels {a} {b} {n}: got {labels:?} want {want_labels:?}", c["kind"]);
        }
    }
    eprintln!("{cases} cases: {tick_bad} tick lists, {nice_bad} nice domains, {label_bad} label lists differ");
    assert_eq!((tick_bad, nice_bad, label_bad), (0, 0, 0));
}
