use std::fmt::Write;

use ql_mir::MirModule;

use crate::{BorrowckResult, LocalEventKind, LocalState, MoveCertainty, MoveReason};

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
        },
    }
}
