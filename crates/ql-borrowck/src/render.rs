use std::fmt::Write;

use ql_mir::MirModule;

use crate::{
    BorrowckResult, ClosureEscapeKind, LocalEventKind, LocalState, MoveCertainty, MoveReason,
};

pub fn render_result(result: &BorrowckResult, mir: &MirModule) -> String {
    let mut output = String::new();

    for (index, body) in result.bodies().iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }

        let _ = writeln!(output, "ownership {}", body.name);
        if let Some(mir_body) = mir.body_for_owner(body.owner) {
            let _ = writeln!(output, "  locals:");
            for (local_index, local) in mir_body.locals().iter().enumerate() {
                let _ = writeln!(output, "    l{local_index} {}", local.name);
            }
        }

        let _ = writeln!(output, "  blocks:");
        for block in &body.blocks {
            let _ = writeln!(
                output,
                "    bb{} in=[{}] out=[{}]",
                block.block_index,
                render_states(&block.entry_states),
                render_states(&block.exit_states)
            );
        }

        let _ = writeln!(output, "  events:");
        for event in &body.events {
            let _ = writeln!(
                output,
                "    l{} {} @ {}..{}",
                event.local.index(),
                render_event(&event.kind),
                event.span.start,
                event.span.end
            );
        }

        let _ = writeln!(output, "  closures:");
        if let Some(mir_body) = mir.body_for_owner(body.owner) {
            for facts in &body.closures {
                let closure = mir_body.closure(facts.closure);
                let escapes = if facts.escapes.is_empty() {
                    "local-only".to_owned()
                } else {
                    facts
                        .escapes
                        .iter()
                        .map(|escape| {
                            format!(
                                "{}@{}..{}",
                                render_closure_escape_kind(&escape.kind),
                                escape.span.start,
                                escape.span.end
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let _ = writeln!(
                    output,
                    "    cl{} captures=[{}] escapes=[{}]",
                    facts.closure.index(),
                    closure
                        .captures
                        .iter()
                        .map(|capture| format!(
                            "l{}:{}@{}..{}",
                            capture.local.index(),
                            mir_body.local(capture.local).name,
                            capture.span.start,
                            capture.span.end
                        ))
                        .collect::<Vec<_>>()
                        .join(", "),
                    escapes
                );
            }
        }
    }

    output
}

fn render_states(states: &[LocalState]) -> String {
    states
        .iter()
        .enumerate()
        .map(|(index, state)| format!("l{index}={}", render_state(state)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_state(state: &LocalState) -> String {
    match state {
        LocalState::Unavailable => "unavailable".to_owned(),
        LocalState::Available => "available".to_owned(),
        LocalState::Moved(info) => {
            let certainty = match info.certainty {
                MoveCertainty::Definite => "moved",
                MoveCertainty::Maybe => "maybe-moved",
            };
            let reasons = info
                .origins
                .iter()
                .map(|origin| match &origin.reason {
                    MoveReason::MoveSelfMethod { method_name } => method_name.clone(),
                    MoveReason::MoveClosureCapture => "move-closure".to_owned(),
                    MoveReason::AwaitTaskHandle => "await-task".to_owned(),
                    MoveReason::SpawnTaskHandle => "spawn-task".to_owned(),
                    MoveReason::CallTaskHandleArgument => "call-task".to_owned(),
                })
                .collect::<Vec<_>>()
                .join("|");
            format!("{certainty}({reasons})")
        }
    }
}

fn render_event(kind: &LocalEventKind) -> String {
    match kind {
        LocalEventKind::Read => "read".to_owned(),
        LocalEventKind::Write => "write".to_owned(),
        LocalEventKind::Consume(reason) => match reason {
            MoveReason::MoveSelfMethod { method_name } => {
                format!("consume(move self {method_name})")
            }
            MoveReason::MoveClosureCapture => "consume(move closure capture)".to_owned(),
            MoveReason::AwaitTaskHandle => "consume(await task handle)".to_owned(),
            MoveReason::SpawnTaskHandle => "consume(spawn task handle)".to_owned(),
            MoveReason::CallTaskHandleArgument => "consume(call task handle argument)".to_owned(),
        },
    }
}

fn render_closure_escape_kind(kind: &ClosureEscapeKind) -> String {
    match kind {
        ClosureEscapeKind::Return => "return".to_owned(),
        ClosureEscapeKind::CallArgument => "call-arg".to_owned(),
        ClosureEscapeKind::CallCallee => "call-callee".to_owned(),
        ClosureEscapeKind::CapturedByClosure { outer } => {
            format!("captured-by-cl{}", outer.index())
        }
    }
}
