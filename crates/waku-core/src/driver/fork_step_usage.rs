//! [fork] token speed: times each main-thread model call from its first
//! streamed output to its end, so the client can divide the provider's exact
//! output tokens by real generation time. Timing lives here rather than in the
//! client because the client's event pump coalesces deltas into ~120 ms
//! commits, which would blur short steps.

use std::time::Instant;

use crate::model::DriverEvent;

#[derive(Default)]
pub(super) struct StepTimer {
    first_output: Option<Instant>,
    saw_reasoning: bool,
}

impl StepTimer {
    /// A model call started; drop any timing left by an interrupted one.
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    /// The current model call streamed output (text or tool input).
    pub(super) fn mark_output(&mut self) {
        self.first_output.get_or_insert_with(Instant::now);
    }

    /// The current model call streamed reasoning text.
    pub(super) fn mark_reasoning(&mut self) {
        self.saw_reasoning = true;
        self.mark_output();
    }

    /// Whether reasoning streamed during this call. Providers that can hide
    /// reasoning use it to decide whether reasoning tokens fall inside the
    /// timed window.
    pub(super) fn saw_reasoning(&self) -> bool {
        self.saw_reasoning
    }

    /// The model call ended with `output_tokens`. `None` when nothing streamed
    /// or the provider reported no output, so there is no rate to measure.
    pub(super) fn finish(&mut self, output_tokens: Option<u64>) -> Option<DriverEvent> {
        let first_output = std::mem::take(self).first_output?;
        let output_tokens = output_tokens.filter(|tokens| *tokens > 0)?;
        Some(DriverEvent::ForkStepUsage {
            output_tokens,
            generation_ms: u64::try_from(first_output.elapsed().as_millis()).unwrap_or(u64::MAX),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finish_needs_streamed_output_and_reported_tokens() {
        let mut timer = StepTimer::default();
        assert!(timer.finish(Some(10)).is_none());

        timer.mark_output();
        assert!(timer.finish(Some(0)).is_none());
        // The failed finish still closed the step.
        assert!(timer.finish(Some(10)).is_none());

        timer.mark_reasoning();
        assert!(timer.saw_reasoning());
        assert!(matches!(
            timer.finish(Some(42)),
            Some(DriverEvent::ForkStepUsage {
                output_tokens: 42,
                ..
            })
        ));
        assert!(!timer.saw_reasoning());
    }
}
