//! A two-point sketch per solver, then a conflicting one: what each API
//! reports for status, DoF, and the redundant and conflicting sets.

pub fn sketch_solver() {
    use sketch_solver::{Along, Constraint, Sketch};
    let build =
        |second: Option<f64>| -> Result<sketch_solver::Solution, sketch_solver::ModelError> {
            let mut s = Sketch::new();
            let o = s.point(Some([0.0, 0.0]))?;
            let a = s.point(Some([3.0, 1.0]))?;
            s.add(Constraint::Fix {
                entity: o,
                at: None,
            })?;
            s.add(Constraint::Distance {
                a: o,
                b: a,
                value: 5.0,
                along: Along::Direct,
            })?;
            if let Some(v) = second {
                s.add(Constraint::Distance {
                    a: o,
                    b: a,
                    value: v,
                    along: Along::Direct,
                })?;
            }
            Ok(s.solve())
        };
    for (name, second) in [
        ("free", None),
        ("redundant", Some(5.0)),
        ("conflict", Some(6.0)),
    ] {
        match build(second) {
            Ok(sol) => println!(
                "{name}: status={:?} dof={} redundant={:?} conflicts={:?} free={}",
                sol.status,
                sol.dof,
                sol.redundant.iter().map(|d| d.source).collect::<Vec<_>>(),
                sol.conflicts.iter().map(|d| d.source).collect::<Vec<_>>(),
                sol.free.len()
            ),
            Err(e) => println!("{name}: model error {e:?}"),
        }
    }
}

pub fn acs_solver() {
    let sketch = |second: Option<f64>| {
        let mut prims = vec![
            r#"{"id":"o","type":"point","x":0,"y":0,"fixed":true}"#.to_string(),
            r#"{"id":"a","type":"point","x":3,"y":1}"#.to_string(),
            r#"{"id":"k1","type":"distance","a":"o","b":"a","value":5}"#.to_string(),
        ];
        if let Some(v) = second {
            prims.push(format!(
                r#"{{"id":"k2","type":"distance","a":"o","b":"a","value":{v}}}"#
            ));
        }
        format!(r#"{{"version":1,"primitives":[{}]}}"#, prims.join(","))
    };
    for (name, second) in [
        ("free", None),
        ("redundant", Some(5.0)),
        ("conflict", Some(6.0)),
    ] {
        let r = acs::sketch_solve::solve_sketch_json(&sketch(second)).unwrap_or_else(|e| e);
        let v: serde_json::Value = serde_json::from_str(&r).unwrap_or_default();
        println!(
            "{name}: status={} dof={} redundant={} conflicting={}",
            v["status"], v["dof"], v["redundant"], v["conflicting"]
        );
    }
}

pub fn ezpz() {
    use ezpz::datatypes::inputs::DatumPoint;
    use ezpz::{Config, Constraint, ConstraintRequest, IdGenerator, solve_analysis};
    for (name, second) in [
        ("free", None),
        ("redundant", Some(5.0)),
        ("conflict", Some(6.0)),
    ] {
        let mut ids = IdGenerator::default();
        let o = DatumPoint::new(&mut ids);
        let a = DatumPoint::new(&mut ids);
        let top = ConstraintRequest::highest_priority;
        let mut reqs = vec![
            top(Constraint::Fixed(o.id_x(), 0.0)),
            top(Constraint::Fixed(o.id_y(), 0.0)),
            top(Constraint::Distance(o, a, 5.0)),
        ];
        if let Some(v) = second {
            reqs.push(top(Constraint::Distance(o, a, v)));
        }
        let guesses = vec![
            (o.id_x(), 0.0),
            (o.id_y(), 0.0),
            (a.id_x(), 3.0),
            (a.id_y(), 1.0),
        ];
        match solve_analysis(&reqs, guesses, Config::default()) {
            Ok(r) => println!(
                "{name}: satisfied={} unsatisfied={:?} underconstrained={:?}",
                r.outcome.is_satisfied(),
                r.outcome.unsatisfied(),
                r.analysis.underconstrained()
            ),
            Err(f) => println!("{name}: failure {}", f.error),
        }
    }
}
