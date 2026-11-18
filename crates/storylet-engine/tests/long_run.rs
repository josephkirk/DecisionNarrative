#[path = "support/mod.rs"]
mod support;

use std::time::{Duration, Instant};

use storylet_engine::chrono_participants::Controller;
use storylet_engine::{Authority, RunAction, RunKernel, RunStart};

fn wait_steps(steps: usize) -> Vec<Duration> {
    let mut content = support::weirs_crossing::content_all_humans();
    content.clock = None;
    content.initial_world.clock = None;
    let mut run = RunKernel::start(&content, RunStart::Beginning, 3).unwrap();
    let actor = (0..run.world().participants.len() as u16)
        .find(|p| {
            matches!(
                run.world().participants.controller[*p as usize],
                Controller::Human { .. }
            )
        })
        .unwrap();
    let seat = match run.world().participants.controller[actor as usize] {
        Controller::Human { seat } => seat,
        Controller::Rule => unreachable!(),
    };
    let mut timings = Vec::with_capacity(steps);
    for i in 0..steps {
        let proposal =
            support::actions::proposal(&run, actor, RunAction::Wait, &format!("wait-{i}"));
        let started = Instant::now();
        run.propose(&content, Authority::Human { seat }, proposal)
            .unwrap();
        timings.push(started.elapsed());
    }
    timings
}

fn mean(window: &[Duration]) -> Duration {
    window.iter().sum::<Duration>() / window.len() as u32
}

#[test]
#[ignore = "measurement; run with --ignored --nocapture"]
fn per_step_cost_stays_flat_over_a_long_run() {
    let timings = wait_steps(500);
    let early = mean(&timings[50..100]);
    let late = mean(&timings[450..500]);
    println!(
        "early mean {early:?}, late mean {late:?}, ratio {:.2}",
        late.as_secs_f64() / early.as_secs_f64()
    );
}
